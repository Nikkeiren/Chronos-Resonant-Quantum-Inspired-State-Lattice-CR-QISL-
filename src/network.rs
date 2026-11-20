//! Asynchronous message network: per-link latency, packet loss, partition and
//! healing, and byte accounting.
//!
//! Randomness is *counter-based*: the loss / latency draw of a message depends
//! only on (network seed, sender, receiver, send tick, salt), never on how many
//! messages other variants or nodes happened to send. Variants therefore see
//! identical channel conditions wherever they send on the same link and tick.

use crate::config::NetworkConfig;
use crate::rng::{hash_combine, mix64, unit_f64};
use serde::Serialize;

pub const DEC_NONE: u8 = 255;
pub const DEC_CONFLICT: u8 = 254;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MsgKind {
    State,
    Heartbeat,
    Witness,
    Reconcile,
}

impl MsgKind {
    pub fn code(&self) -> u64 {
        match self {
            MsgKind::State => 1,
            MsgKind::Heartbeat => 2,
            MsgKind::Witness => 3,
            MsgKind::Reconcile => 4,
        }
    }
}

/// Wire accounting model (ASSUMPTION A9): 12-byte header; amplitudes and phases
/// counted as 4-byte floats; 1 decision byte; witness payload 11 bytes.
pub const HEADER_BYTES: u64 = 12;

pub fn message_bytes(kind: MsgKind, k: usize) -> u64 {
    match kind {
        MsgKind::State | MsgKind::Heartbeat | MsgKind::Reconcile => HEADER_BYTES + 1 + 8 * k as u64,
        MsgKind::Witness => HEADER_BYTES + 11,
    }
}

#[derive(Clone, Debug)]
pub struct Message {
    pub from: usize,
    pub to: usize,
    pub kind: MsgKind,
    pub send_tick: u64,
    pub seq: u64,
    pub amp: Vec<f64>,
    pub phase: Vec<f64>,
    pub decision: u8,
    pub w_origin: usize,
    pub w_id: u64,
    pub w_a: u8,
    pub w_b: u8,
    pub ttl: u8,
}

impl Message {
    #[allow(clippy::too_many_arguments)]
    pub fn state(from: usize, to: usize, kind: MsgKind, tick: u64, seq: u64, amp: &[f64], phase: &[f64], decision: u8) -> Message {
        Message {
            from,
            to,
            kind,
            send_tick: tick,
            seq,
            amp: amp.to_vec(),
            phase: phase.to_vec(),
            decision,
            w_origin: 0,
            w_id: 0,
            w_a: 0,
            w_b: 0,
            ttl: 0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn witness(from: usize, to: usize, tick: u64, origin: usize, id: u64, a: u8, b: u8, ttl: u8) -> Message {
        Message {
            from,
            to,
            kind: MsgKind::Witness,
            send_tick: tick,
            seq: 0,
            amp: Vec::new(),
            phase: Vec::new(),
            decision: DEC_NONE,
            w_origin: origin,
            w_id: id,
            w_a: a,
            w_b: b,
            ttl,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct Counters {
    pub sent: u64,
    pub delivered: u64,
    pub dropped_loss: u64,
    pub dropped_partition: u64,
    /// Rejected by receiver-side input validation (counted after delivery).
    pub dropped_invalid: u64,
    /// Scheduled for a tick beyond the horizon (never delivered).
    pub undelivered_end: u64,
    pub bytes_sent: u64,
    pub bytes_delivered: u64,
    pub state_updates: u64,
    pub heartbeats: u64,
    pub witness_messages: u64,
    pub reconciliation_messages: u64,
    pub adversary_messages: u64,
    /// Not implemented (no retransmission layer); always 0.
    pub retransmissions: u64,
    /// Not implemented (no authentication); always 0.
    pub signature_ops: u64,
    /// Stale / reordered messages ignored by the receiver cache.
    pub stale_ignored: u64,
}

pub struct Network {
    pub n: usize,
    pub k: usize,
    seed: u64,
    cfg: NetworkConfig,
    pub groups: Vec<u8>,
    buckets: Vec<Vec<Message>>,
    pub counters: Counters,
}

impl Network {
    pub fn new(n: usize, k: usize, seed: u64, cfg: NetworkConfig, groups: Vec<u8>, horizon: u64) -> Network {
        let size = horizon as usize + 1;
        Network {
            n,
            k,
            seed: mix64(seed ^ 0x4E45_5457_4F52_4B21),
            cfg,
            groups,
            buckets: vec![Vec::new(); size],
            counters: Counters::default(),
        }
    }

    pub fn partition_active(&self, tick: u64) -> bool {
        self.cfg.partition_enabled && tick >= self.cfg.partition_start && tick < self.cfg.partition_end
    }

    fn crosses_partition(&self, from: usize, to: usize, tick: u64) -> bool {
        self.partition_active(tick) && self.groups[from] != self.groups[to]
    }

    fn latency_ticks(&self, u: f64, from: usize, to: usize) -> u64 {
        let c = &self.cfg;
        let base = match c.latency_kind.as_str() {
            "fixed" => c.latency_min,
            "exponential" => c.latency_min - c.latency_mean_extra * (1.0 - u).max(1e-12).ln(),
            _ => c.latency_min + (u * (c.latency_max - c.latency_min + 1.0)).floor(),
        };
        let mult = if from > to { c.asymmetry } else { 1.0 };
        let t = (base * mult).round();
        let t = if t.is_finite() { t } else { 1.0 };
        (t.max(1.0) as u64).min(c.latency_cap.max(1))
    }

    /// Send a message at `tick`. `salt` distinguishes several messages on the same link and tick.
    pub fn send(&mut self, msg: Message, tick: u64, salt: u64) {
        let bytes = message_bytes(msg.kind, self.k);
        self.counters.sent += 1;
        self.counters.bytes_sent += bytes;
        match msg.kind {
            MsgKind::State => self.counters.state_updates += 1,
            MsgKind::Heartbeat => self.counters.heartbeats += 1,
            MsgKind::Witness => self.counters.witness_messages += 1,
            MsgKind::Reconcile => self.counters.reconciliation_messages += 1,
        }
        if self.crosses_partition(msg.from, msg.to, tick) {
            self.counters.dropped_partition += 1;
            return;
        }
        let link = hash_combine(self.seed, (msg.from as u64) * (self.n as u64) + msg.to as u64);
        let h = hash_combine(hash_combine(link, tick), salt.wrapping_mul(8).wrapping_add(msg.kind.code()));
        let u_loss = unit_f64(mix64(h ^ 0xA5A5_A5A5_A5A5_A5A5));
        if u_loss < self.cfg.packet_loss {
            self.counters.dropped_loss += 1;
            return;
        }
        let u_lat = unit_f64(mix64(h ^ 0x5A5A_5A5A_5A5A_5A5A));
        let deliver = tick + self.latency_ticks(u_lat, msg.from, msg.to);
        if (deliver as usize) >= self.buckets.len() {
            self.counters.undelivered_end += 1;
            return;
        }
        self.buckets[deliver as usize].push(msg);
    }

    /// Messages due at `tick`. In-flight messages that cross an active partition are lost.
    pub fn take_deliveries(&mut self, tick: u64) -> Vec<Message> {
        let due = std::mem::take(&mut self.buckets[tick as usize]);
        let mut out = Vec::with_capacity(due.len());
        for m in due {
            if self.crosses_partition(m.from, m.to, tick) {
                self.counters.dropped_partition += 1;
            } else {
                self.counters.delivered += 1;
                self.counters.bytes_delivered += message_bytes(m.kind, self.k);
                out.push(m);
            }
        }
        out
    }

    /// Messages still queued for future ticks (after the last processed tick).
    pub fn in_flight(&self) -> u64 {
        self.buckets.iter().map(|b| b.len() as u64).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net(loss: f64, partition: bool) -> Network {
        let cfg = NetworkConfig {
            packet_loss: loss,
            partition_enabled: partition,
            partition_start: 5,
            partition_end: 10,
            ..NetworkConfig::default()
        };
        let groups = vec![0u8, 0, 1, 1];
        Network::new(4, 2, 99, cfg, groups, 50)
    }

    fn msg(from: usize, to: usize, tick: u64) -> Message {
        Message::state(from, to, MsgKind::State, tick, 1, &[0.5, 0.5], &[0.0, 0.0], DEC_NONE)
    }

    #[test]
    fn latency_is_at_least_one_and_bounded() {
        let n = net(0.0, false);
        for i in 0..1000 {
            let u = (i as f64) / 1000.0;
            let t = n.latency_ticks(u, 0, 1);
            assert!((1..=5).contains(&t), "latency {} out of [1,5]", t);
        }
    }

    #[test]
    fn asymmetry_scales_reverse_direction() {
        let cfg = NetworkConfig {
            latency_kind: "fixed".into(),
            latency_min: 2.0,
            latency_max: 2.0,
            asymmetry: 3.0,
            ..NetworkConfig::default()
        };
        let n = Network::new(3, 2, 1, cfg, vec![0, 0, 0], 20);
        assert_eq!(n.latency_ticks(0.3, 0, 1), 2);
        assert_eq!(n.latency_ticks(0.3, 1, 0), 6);
    }

    #[test]
    fn loss_rate_matches_probability() {
        let mut n = net(0.2, false);
        for t in 0..40u64 {
            for s in 0..200u64 {
                n.send(msg(0, 1, t), t, s);
            }
        }
        let rate = n.counters.dropped_loss as f64 / n.counters.sent as f64;
        assert!((rate - 0.2).abs() < 0.03, "rate {}", rate);
    }

    #[test]
    fn counter_based_draws_are_order_independent() {
        // The loss outcome of a message on link 0->1 must not depend on other traffic.
        let mut a = net(0.5, false);
        let mut b = net(0.5, false);
        let mut lost_a = Vec::new();
        let mut lost_b = Vec::new();
        for t in 0..200u64 {
            let before = a.counters.dropped_loss;
            a.send(msg(0, 1, t), t, 7);
            lost_a.push(a.counters.dropped_loss - before);
            b.send(msg(2, 3, t), t, 0); // unrelated traffic first
            let before = b.counters.dropped_loss;
            b.send(msg(0, 1, t), t, 7);
            lost_b.push(b.counters.dropped_loss - before);
        }
        assert_eq!(lost_a, lost_b);
        assert!(lost_a.iter().any(|&x| x == 1) && lost_a.iter().any(|&x| x == 0));
    }

    #[test]
    fn partition_blocks_cross_group_and_counts() {
        let mut n = net(0.0, true);
        n.send(msg(0, 2, 6), 6, 0); // crosses, partition active
        n.send(msg(0, 1, 6), 6, 0); // same group
        assert_eq!(n.counters.dropped_partition, 1);
        n.send(msg(0, 2, 12), 12, 0); // after healing
        assert_eq!(n.counters.dropped_partition, 1);
        assert_eq!(n.counters.sent, 3);
    }

    #[test]
    fn in_flight_message_lost_when_partition_starts() {
        let mut n = net(0.0, true);
        // sent at tick 3 (before partition), latency forced long enough to arrive during partition
        let cfg = NetworkConfig {
            packet_loss: 0.0,
            latency_kind: "fixed".into(),
            latency_min: 5.0,
            latency_max: 5.0,
            partition_enabled: true,
            partition_start: 5,
            partition_end: 10,
            ..NetworkConfig::default()
        };
        n.cfg = cfg;
        n.send(msg(0, 2, 3), 3, 0);
        assert_eq!(n.counters.dropped_partition, 0);
        let got = n.take_deliveries(8);
        assert!(got.is_empty());
        assert_eq!(n.counters.dropped_partition, 1);
    }

    #[test]
    fn accounting_identity() {
        let mut n = net(0.3, true);
        for t in 0..30u64 {
            for s in 0..20u64 {
                n.send(msg((s % 4) as usize, ((s + 1) % 4) as usize, t), t, s);
            }
            let _ = n.take_deliveries(t);
        }
        let c = &n.counters;
        assert_eq!(c.sent, c.delivered + c.dropped_loss + c.dropped_partition + c.undelivered_end + n.in_flight());
    }

    #[test]
    fn byte_model() {
        assert_eq!(message_bytes(MsgKind::State, 2), 12 + 1 + 16);
        assert_eq!(message_bytes(MsgKind::Witness, 2), 23);
    }
}
