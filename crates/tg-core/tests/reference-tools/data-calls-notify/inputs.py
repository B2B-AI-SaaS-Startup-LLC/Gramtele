import json,sys
cases=['not-json','[]','{}','{"columns":[]}','{"columns":[null]}','{"columns":[["x"]]}']
for ys in [[0],[5],[-5],[0,0,0],[-1,-2,-3],[1,8,2],[-.5,.5,2.5],[None,True,"4"]]:
 n=len(ys);cases.append({'columns':[['x',*range(n)],['y0',*ys]]})
for extra in [{},{'subchart':{'show':False,'defaultZoom':[2,0]}},{'subchart':{'show':'false','defaultZoom':[99]}},{'percentage':True,'xTooltipFormatter':"date 'week'"},{'yTickFormatter':'TON XTR'},{'yTickFormatter':'XTR'},{'hidden':['b',None],'colors':{'a':'light#112233','b':'first#bad#abc'},'names':{'a':'one-two-three','b':'а-б'}},{'colors':{'a':'bad color','b':12}},{'colors':{'a':'first\nsecond#abc\nlast#fff'}}]:
 cases.append({'columns':[['x',0,1,2],['a',1,5,3],['b',2,0,-2]],**extra})
cases.extend([{'columns':[['y',1,2,3],['x',100,200,300]]},{'columns':[['x',0,1],['a',1]]},{'columns':[['x',0],['x',1],['a',2,3]]},{'columns':[['x',1,1],['a',3,3]]},{'columns':[['x',0,1],['a'],['a',3,4]]},{'columns':[['x',0,1000,5000],['a',1,2,3]]}])
cases.append('\ufeff{"columns":[["x",0,1],["a",1,2]]}')
with open(sys.argv[1],'w') as f:
 for c in cases:f.write((json.dumps(c,ensure_ascii=False,separators=(',',':')) if not isinstance(c,str) else c)+'\n')
