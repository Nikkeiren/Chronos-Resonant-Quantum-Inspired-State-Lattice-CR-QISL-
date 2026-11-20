# Behavioural port of the Rust engine logic (node.rs/model.rs/sim.rs) used ONLY to sanity-check
# qualitative behaviour asserted by the integration tests. RNG streams differ from the Rust code.
import numpy as np, math, sys
NONE, CONF = 255, 254
FEAT = {'A':dict(damp=0,phase=0,integ=0,wit=0,rec=0),'B':dict(damp=1,phase=0,integ=0,wit=0,rec=0),
        'C':dict(damp=1,phase=0,integ=1,wit=1,rec=1),'D':dict(damp=1,phase=1,integ=1,wit=1,rec=1)}
def cd(a,b):
    d=(a-b)%(2*math.pi); return 2*math.pi-d if d>math.pi else d
def run(variant, seed, n=40, K=2, T=160, ps=30, pe=80, loss=0.2, lat=(1,5), alpha=.5, beta=.5, rate=.5, hb=10, ta=.01, tth=.05, ttl=30,
        tm=.04, sm=.08, em=.04, W=12, eps=.01, eta=.1, kappa=8., minw=.05, gam=.5, wttl=8, wcool=10, rper=5, boost=1.5, rmin=10, rhold=8,
        inj=31, inj_s=.6, k=6, prw=.1, frac=.65, adv_frac=0.0, validate=True, noise=.02, hold=10, conv_eps=.02):
    F=FEAT[variant]; rng=np.random.default_rng(seed); rng2=np.random.default_rng(seed+1000)
    adj=[set() for _ in range(n)]
    for i in range(n):
        for d in range(1,k//2+1): j=(i+d)%n; adj[i].add(j); adj[j].add(i)
    for i in range(n):
        for d in range(1,k//2+1):
            j=(i+d)%n
            if rng.random()<prw and j in adj[i]:
                c=int(rng.integers(n))
                if c!=i and c not in adj[i]: adj[i].discard(j); adj[j].discard(i); adj[i].add(c); adj[c].add(i)
    adj=[sorted(a) for a in adj]
    order=[];seen=[0]*n;q=[0];seen[0]=1
    while q:
        u=q.pop(0);order.append(u)
        for v in adj[u]:
            if not seen[v]: seen[v]=1;q.append(v)
    order+= [u for u in range(n) if not seen[u]]
    s0=math.ceil(frac*n); grp=[1]*n
    for u in order[:s0]: grp[u]=0
    advs=set(rng.permutation(n)[:int(round(adv_frac*n))].tolist()) if adv_frac>0 else set()
    amp=[np.abs(1/K+rng.normal(0,noise,K)) for _ in range(n)]; amp=[a/a.sum() for a in amp]
    ph=[np.zeros(K) for _ in range(n)]
    cache=[{j:None for j in adj[i]} for i in range(n)]; wts=[{j:1.0 for j in adj[i]} for i in range(n)]
    st=dict(seq=[0]*n,lsa=[None]*n,lsp=[np.zeros(K)]*n,lsd=[NONE]*n,lst=[None]*n,lrt=[None]*n,state=[0]*n,dec=[NONE]*n,dom=[NONE]*n,ldom=[NONE]*n,
            cnt=[0]*n,csince=[None]*n,rc=[0]*n,rd=[NONE]*n,tof=[False]*n,seen=[set() for _ in range(n)],nw=[0]*n,wcu=[0]*n,fst=[None]*n,
            flips=[0]*n,rev=[0]*n,cent=[0]*n)
    buckets={}; C=dict(sent=0,deliv=0,dl=0,dp=0,di=0,wit=0,rec=0,hbc=0,stu=0)
    def active(t): return ps<=t<pe
    def send(frm,to,t,msg,kind):
        C['sent']+=1
        C[{'S':'stu','H':'hbc','W':'wit','R':'rec'}[kind]]+=1
        if active(t) and grp[frm]!=grp[to]: C['dp']+=1; return
        if rng2.random()<loss: C['dl']+=1; return
        L=int(rng2.integers(lat[0],lat[1]+1))
        if t+L<T: buckets.setdefault(t+L,[]).append((frm,to,kind,msg))
    def enter_conflict(i,t):
        st['state'][i]=3; st['csince'][i]=t; st['cent'][i]+=1; st['rc'][i]=0; st['rd'][i]=NONE; st['tof'][i]=False; st['dec'][i]=NONE; st['cnt'][i]=0
    agree=[];div=[];conv=[];hist=[];quor=[]
    for t in range(T):
        for (frm,to,kind,msg) in buckets.pop(t,[]):
            if active(t) and grp[frm]!=grp[to]: C['dp']+=1; continue
            C['deliv']+=1
            if to in advs: continue
            if kind=='W':
                origin,wid,a,b,tl=msg
                if (origin,wid) in st['seen'][to]: continue
                st['seen'][to].add((origin,wid))
                if F['wit'] and st['state'][to]==2 and st['dec'][to] in (a,b): enter_conflict(to,t)
                if tl>1:
                    for nb in adj[to]:
                        if nb!=frm: send(to,nb,t,(origin,wid,a,b,tl-1),'W')
            else:
                a,p,d,stk,sq=msg
                c=cache[to][frm]
                if c is None or sq>c[4]: cache[to][frm]=(a,p,d,t,sq)
        if t==inj:
            for i in range(n):
                if i in advs: continue
                c=grp[i]; e=np.zeros(K); e[c]=1; amp[i]=(1-inj_s)*amp[i]+inj_s*e; amp[i]/=amp[i].sum()
                if F['phase']: ph[i][c]= 0.0 if c==0 else 1.5707963
        for i in range(n):
            if i in advs: continue
            idx=[j for j in adj[i] if cache[i][j] is not None and t-cache[i][j][3]<=ttl]
            S=[(cache[i][j][0],cache[i][j][1],wts[i][j] if F['integ'] else 1.0) for j in idx]
            if F['integ'] and len(S)>=2:
                med=np.array([np.median([s[0][c] for s in S]+[amp[i][c]]) for c in range(K)])
                pm=np.zeros(K)
                if F['phase']:
                    for c in range(K):
                        cands=[s[1][c] for s in S]+[ph[i][c]]
                        pm[c]=min(cands,key=lambda x:sum(cd(x,y) for y in cands))
                for n_,j in enumerate(idx):
                    s=S[n_]; d=0.5*np.abs(s[0]-med).sum()
                    if F['phase']: d+=gam*sum(med[c]*cd(s[1][c],pm[c])/math.pi for c in range(K))
                    tt=math.exp(-kappa*d); w=(1-eta)*wts[i][j]+eta*tt; w=min(1,max(minw,w)); wts[i][j]=w; S[n_]=(s[0],s[1],w)
            step=rate*((1-alpha) if F['damp'] else 1.0)
            if st['state'][i]==3 and F['rec']: step*=boost
            step=min(step,1.0)
            prev=amp[i].copy()
            ws=sum(s[2] for s in S)
            if not S or not ws>0: na=amp[i].copy(); np_=ph[i].copy()
            else:
                na=np.zeros(K); np_=np.zeros(K)
                for c in range(K):
                    am=sum(s[2]/ws*s[0][c] for s in S)
                    if F['phase']:
                        re=sum(s[2]/ws*s[0][c]*math.cos(s[1][c]) for s in S); im=sum(s[2]/ws*s[0][c]*math.sin(s[1][c]) for s in S)
                        mag=math.hypot(re,im); tamp=(1-beta)*am+beta*mag; tph=math.atan2(im,re) if mag>1e-12 else ph[i][c]
                        ore=amp[i][c]*math.cos(ph[i][c]); oim=amp[i][c]*math.sin(ph[i][c])
                        nre=ore+step*(tamp*math.cos(tph)-ore); nim=oim+step*(tamp*math.sin(tph)-oim)
                        nm=math.hypot(nre,nim); na[c]=nm; np_[c]=math.atan2(nim,nre) if nm>1e-12 else tph
                    else: na[c]=amp[i][c]+step*(am-amp[i][c])
            if F['phase']: np_=np_%(2*math.pi)
            na=na/na.sum(); amp[i]=na; ph[i]=np_
            top=int(np.argmax(na)); srt=np.sort(na); mg=srt[-1]-srt[-2]
            nd=top if mg>=tm else NONE
            if nd!=NONE:
                if st['ldom'][i]!=NONE and nd!=st['ldom'][i]: st['flips'][i]+=1
                st['ldom'][i]=nd
            md=np.abs(na-prev).max(); pd_=st['dom'][i]; st['dom'][i]=nd
            sv=st['state'][i]
            if sv==2:
                if nd!=st['dec'][i] or mg<em:
                    st['rev'][i]+=1; st['state'][i]=1 if nd!=NONE else 0; st['dec'][i]=NONE; st['cnt'][i]=0
            elif sv!=3:
                if nd==NONE: st['state'][i]=0; st['cnt'][i]=0
                else:
                    st['state'][i]=1
                    st['cnt'][i]= st['cnt'][i]+1 if (mg>=sm and nd==pd_ and md<=eps) else 0
                    if st['cnt'][i]>=W:
                        st['state'][i]=2; st['dec'][i]=nd
                        if st['fst'][i] is None: st['fst'][i]=t
            wout=None
            if F['wit'] and st['state'][i]==2 and t>=st['wcu'][i]:
                other=None
                for j in adj[i]:
                    c=cache[i][j]
                    if c is not None and t-c[3]<=ttl and c[2]<CONF and c[2]!=st['dec'][i]: other=c[2]; break
                if other is not None:
                    wid=st['nw'][i]; st['nw'][i]+=1; st['seen'][i].add((i,wid)); st['wcu'][i]=t+wcool
                    a=st['dec'][i]; enter_conflict(i,t); wout=(wid,a,other)
            if st['state'][i]==3:
                age=t-(st['csince'][i] if st['csince'][i] is not None else t)
                if age>=80 and not st['tof'][i]: st['tof'][i]=True
                if nd!=NONE and mg>=sm and age>=rmin:
                    if nd==st['rd'][i]: st['rc'][i]+=1
                    else: st['rd'][i]=nd; st['rc'][i]=1
                else: st['rc'][i]=0; st['rd'][i]=NONE
                if st['rc'][i]>=rhold:
                    blocked=any(cache[i][j] is not None and t-cache[i][j][3]<=ttl and cache[i][j][2]<CONF and cache[i][j][2]!=nd for j in adj[i])
                    if not blocked: st['state'][i]=1; st['cnt'][i]=0; st['dec'][i]=NONE; st['csince'][i]=None; st['rc'][i]=0; st['rd'][i]=NONE
            advd=st['dec'][i] if st['state'][i]==2 else (CONF if st['state'][i]==3 else NONE)
            kind=None
            if F['rec'] and st['state'][i]==3 and (st['lrt'][i] is None or t-st['lrt'][i]>=rper): kind='R'; st['lrt'][i]=t
            elif st['lst'][i] is None: kind='S'
            else:
                gate=np.any(np.abs(amp[i]-st['lsa'][i])>ta) or (F['phase'] and any(cd(ph[i][c],st['lsp'][i][c])>tth for c in range(K)))
                if gate or advd!=st['lsd'][i]: kind='S'
                elif t-st['lst'][i]>=hb: kind='H'
            if kind:
                st['seq'][i]+=1; st['lsa'][i]=amp[i].copy(); st['lsp'][i]=ph[i].copy(); st['lsd'][i]=advd; st['lst'][i]=t
                for nb in adj[i]: send(i,nb,t,(amp[i].copy(),ph[i].copy(),advd,t,st['seq'][i]),'S' if kind=='R' and False else ('H' if kind=='H' else ('R' if kind=='R' else 'S')))
            if wout:
                for nb in adj[i]: send(i,nb,t,(i,wout[0],wout[1],wout[2],wttl),'W')
        H=[i for i in range(n) if i not in advs]
        stab=[sum(1 for i in H if st['state'][i]==2 and st['dec'][i]==c) for c in range(K)]
        tot=sum(stab); mx=max(stab)
        div.append(1-mx/tot if tot else 0.0); agree.append(mx==len(H)); quor.append(mx>=0.9*len(H))
        mean=np.mean([amp[i] for i in H],axis=0); sp=max(np.abs(amp[i]-mean).max() for i in H); conv.append(sp<conv_eps)
        hist.append((t,sum(1 for i in H if st['state'][i]==0),sum(1 for i in H if st['state'][i]==1),stab[0],stab[1],sum(1 for i in H if st['state'][i]==3),round(sp,3)))
    def frs(flags,frm,h):
        run_=0
        for t in range(frm,len(flags)):
            if flags[t]:
                run_+=1
                if run_>=h: return t+1-h
            else: run_=0
        return None
    a=frs(agree,pe,hold); c=frs(conv,pe,hold); qa=frs(quor,pe,hold)
    res=dict(quorum_t=(qa-pe) if qa is not None else None, split_pre=sum(1 for d in div[ps:pe] if d>0), div_pre=np.mean(div[ps:pe]), div_post=np.mean(div[pe:]), agree_t=(a-pe) if a is not None else None, conv_t=(c-pe) if c is not None else None,
             conflicts=sum(st['cent']), witness=C['wit'], recon=C['rec'], hb=C['hbc'], sent=C['sent'], dl=C['dl'], dp=C['dp'], deliv=C['deliv'],
             never=sum(1 for i in range(n) if st['fst'][i] is None), flips=np.mean(st['flips']), rev=sum(st['rev']),
             final_stable=sum(1 for i in range(n) if st['state'][i]==2)/n, final_conf=sum(1 for i in range(n) if st['state'][i]==3))
    return res,hist
if __name__=='__main__':
    seeds=[int(x) for x in sys.argv[1:]] or [1,2,3]
    for s in seeds:
        for v in 'ABCD':
            r,h=run(v,s)
            print(s,v,{k:(round(x,3) if isinstance(x,float) else x) for k,x in r.items()})
