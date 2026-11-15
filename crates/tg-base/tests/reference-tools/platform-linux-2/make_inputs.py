import json,struct,sys

def blob(entries,little=True,serial=1):
 endian='<' if little else '>'
 def number(v,fmt):return struct.pack(endian+fmt,v)
 def pad(v):return v+b'\0'*((-len(v))%4)
 data=bytes([0 if little else 1])+b'\0'*3+number(serial,'I')+number(len(entries),'I')
 for typ,name,change,value in entries:
  data+=bytes([typ,0])+number(len(name),'H')+pad(name)+number(change,'i')
  if typ==0:data+=number(value,'i')
  elif typ==1:data+=number(len(value),'i')+pad(value)
  elif typ==2:data+=number(value[0],'H')+number(value[1],'H')+number(value[2],'H')+number(value[3],'H')
 return data
rows=[]
def add(blobs):rows.append(dict(op='parse',blobs=[b.hex() for b in blobs]))
for little in [True,False]:
 add([blob([],little)])
 for n in range(10):
  add([blob([(0,b'x'*n,0,-2147483648),(1,b's'*n,1,bytes(range(n))),(2,b'c'*n,2,(1,2,3,255))],little)])
 for colors in [(0,0,0,0),(255,255,255,255),(256,1,2,3),(65535,0,0,65535),(1,2,3,256)]:add([blob([(2,b'color',1,colors)],little)])
 entries=[(0,b'a',1,10),(1,b'b',2,b'hello\0world'),(2,b'c',3,(10,20,30,40))]
 raw=blob(entries,little)
 for n in range(len(raw)+1):add([raw[:n]])
 for serial in [-2147483648,-2,-1,0,1,2147483647]:add([blob([(0,b'a',serial,100)],little)])
 add([blob([(0,b'a',5,1)],little,99),blob([(0,b'a',4,2),(1,b'b',0,b'new')],little,0),blob([(0,b'a',5,3)],little),blob([(1,b'a',6,b'replaced')],little)])
 add([blob([(42,b'unknown',1,None),(0,b'a',2,7)],little)])
 add([blob([(0,b'\0\xff\x80name',1,7)],little)])
add([bytes([2])+bytes(11)])
rows.append(dict(op='keys'))
for old in [0,1,2,3,0x20000,0x400000,0x420000,0xffffffff]:
 for mask in [0,1,2,3,0x420000,0xffffffff]:
  for mode in range(3):rows.append(dict(op='mask',old=old,mask=mask,mode=mode))
for n in [-2147483648,-2147484,-2147483,-1,0,1,2147483,2147484,3600000,2147483647]:rows.append(dict(op='scale',value=n))
for n in range(3):rows.extend([dict(op='playback',value=n),dict(op='loop',value=n)])
for event in ['Pause','Play','Stop','PlayPause','Next','Previous','Quit','Raise']:rows.append(dict(op='command',value=event))
for status in ['None','Track','Playlist','invalid','']:rows.append(dict(op='loop_command',value=status))
for n in [0,1,2,3,4,7,8,31,100,101,255]:rows.append(dict(op='mouse_descriptor',value=n))
rows.append(dict(op='abi'))
for row in rows:print(json.dumps(row,separators=(',',':')))
