import math, numpy as np
from scipy import stats, special
def erfc_port(x):
    if x!=x: return float('nan')
    if x<0: return 2.0-erfc_port(-x)
    if x<2.5:
        term=x; s=x; x2=x*x; n=0.0
        while True:
            n+=1.0; term*= -x2/n; add=term/(2*n+1); s+=add
            if abs(add)<1e-17*max(abs(s),1e-300) or n>200: break
        return 1.0-2.0/math.sqrt(math.pi)*s
    t=x
    for n in range(80,0,-1): t=x+(n/2.0)/t
    return math.exp(-x*x)/(math.sqrt(math.pi)*t)
mx=0
for x in np.concatenate([np.linspace(0,8,801),[2.4999,2.5,2.5001]]):
    a=erfc_port(x); b=special.erfc(x)
    rel=abs(a-b)/max(b,1e-300); mx=max(mx,rel)
print("max rel err erfc port:",mx)
def wil(x,y,exact_max=30):
    d=[a-b for a,b in zip(x,y) if math.isfinite(a) and math.isfinite(b)]
    nz=[v for v in d if v!=0.0]; n=len(nz)
    if n==0: return dict(p=1.0)
    ab=np.abs(nz); order=np.argsort(ab,kind='stable'); ranks=np.zeros(n); tie=0.0; has=False
    i=0
    while i<n:
        j=i
        while j+1<n and ab[order[j+1]]==ab[order[i]]: j+=1
        avg=(i+j)/2.0+1.0
        for t in range(i,j+1): ranks[order[t]]=avg
        t_=j-i+1
        if t_>1: has=True; tie+=t_**3-t_
        i=j+1
    wp=sum(r for r,v in zip(ranks,nz) if v>0); wm=sum(r for r,v in zip(ranks,nz) if v<0)
    if n<=exact_max and not has:
        counts=[0]*(n*(n+1)//2+1); counts[0]=1
        for r in range(1,n+1):
            for s in range(len(counts)-1,r-1,-1): counts[s]+=counts[s-r]
        W=int(min(wp,wm)); cum=sum(counts[:W+1]); p=min(1.0,2*cum/2**n); method='exact'; z=float('nan')
    else:
        mean=n*(n+1)/4; var=n*(n+1)*(2*n+1)/24-tie/48
        diff=wp-mean; z=max(abs(diff)-0.5,0)/math.sqrt(var); p=min(1.0,erfc_port(z/math.sqrt(2))); method='normal'
    return dict(p=p,wp=wp,wm=wm,n=n,method=method,rb=(wp-wm)/(wp+wm))
x=[1.83,0.50,1.62,2.48,1.68,1.88,1.55,3.06,1.30]; y=[0.878,0.647,0.598,2.05,1.06,1.29,1.06,3.14,1.29]
r=wil(x,y); print("classic:",r, "scipy exact:",stats.wilcoxon(x,y,method='exact').pvalue)
rng=np.random.default_rng(0)
for trial in range(5):
    n=int(rng.integers(8,60)); a=rng.normal(0.3,1,n); b=rng.normal(0,1,n)
    if trial%2: a=np.round(a,1); b=np.round(b,1)
    mine=wil(list(a),list(b))
    sp=stats.wilcoxon(a,b,method='approx',correction=True,zero_method='wilcox').pvalue if mine['method']=='normal' else stats.wilcoxon(a,b,method='exact').pvalue
    print(trial,n,mine['method'],round(mine['p'],6),round(float(sp),6))
# reference numbers for rust tests
a=[10.0,12,9,15,14,11,13,16,12,10,18,17,9,14,13,15,12,11,10,16,17,14,13,12,11,15,14,10,9,13,12,14,18,17,16,15,14,13,12,11]
b=[a_i-(1.5 if i%3 else -0.5)-(i%5)*0.5+(0.25 if i%2 else 0) for i,a_i in enumerate(a)]
m=wil(a,b); print("tied case:",m, stats.wilcoxon(a,b,method='approx',correction=True).pvalue)
print(a[:5],b[:5])
