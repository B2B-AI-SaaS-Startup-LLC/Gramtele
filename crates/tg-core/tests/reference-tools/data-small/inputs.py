import json,sys
cases=['invalid','[]','{}',{'publicKey':{}},{'publicKey':{'a':1}}]
for timeout in [None,0,-1,1,1.5,60000,2147483647,2147483648,'10']:
 cases.append({'publicKey':{'rp':{'id':'telegram.org','name':'t-г'},'user':{'id':'-_+/QQ==','name':'u','displayName':'d'},'challenge':'-_+/AA!B=C?D','pubKeyCredParams':[{'type':'public-key','alg':-7},None,{'alg':1.5}],'timeout':timeout,'rpId':'rp','userVerification':'required','allowCredentials':[None,{'id':'-__-AA','type':'public-key'}]}})
for value in ['A','AA','AAA','AAAA','AA=AA','AA!AA','-_+/','YWJj===ZGVm','!!!',123,None]:
 cases.append({'publicKey':{'user':{'id':value},'challenge':value,'allowCredentials':[{'id':value}]}})
cases.append('\ufeff{"publicKey":{"timeout":4}}')
with open(sys.argv[1],'w') as f:
 for c in cases:f.write((c if isinstance(c,str)else json.dumps(c,ensure_ascii=False,separators=(',',':')))+'\n')
