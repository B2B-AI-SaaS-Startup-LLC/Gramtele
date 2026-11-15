"""Synthetic TL input corpus from the pinned schema, independent of Rust code."""
from pathlib import Path
import sys,re,struct
root,out=map(Path,sys.argv[1:3]);schema=(root/'SourceFiles/mtproto/scheme/api.tl').read_text()
defs={}; types={}
for line in schema.split('---functions---')[0].splitlines():
 m=re.fullmatch(r'(\w+(?:\.\w+)?)#([0-9a-f]+)\s*(.*?)\s*= ([\w.]+);',line.split('//')[0].strip())
 if not m:continue
 name,cons,params,typ=m.groups();defs[name]=(int(cons,16),params.split(),typ);types.setdefault(typ,[]).append(name)
pref={'Photo':'photo','Document':'document','MessageMedia':'messageMediaPhoto','Message':'message','WebPage':'webPage','StoryItem':'storyItem','MessageExtendedMedia':'messageExtendedMedia','StarGift':'starGiftUnique','StarGiftAttribute':'starGiftAttributeModel','MessageReplyHeader':'messageReplyHeader','MessageAction':'messageActionChatEditPhoto','DraftMessage':'draftMessage','Dialog':'dialog','ForumTopic':'forumTopic','SavedDialog':'monoForumDialog','ChatFull':'chatFull'}
def u32(n):return struct.pack('<I',n&0xffffffff)
def u64(n):return struct.pack('<Q',n&0xffffffffffffffff)
def string(s):
 s=s if isinstance(s,bytes) else s.encode();b=bytes([len(s)])+s;return b+b'\0'*((-len(b))%4)
def val(typ,depth,seed):
 if typ=='int':return u32(17)
 if typ=='long':return u64(17)
 if typ=='double':return struct.pack('<d',1.25)
 if typ in ['string','bytes']:return string('ab')
 if typ=='int128':return b'\1'*16
 if typ=='int256':return b'\1'*32
 m=re.fullmatch(r'Vector<(.+)>',typ)
 if m:return u32(0x1cb5c415)+u32(0 if depth>=4 else 2)+(b'' if depth>=4 else val(m[1],depth+1,seed)+val(m[1],depth+1,seed+1))
 choices=types[typ];name=pref.get(typ,choices[0]) if depth<7 else choices[0]
 if depth>18:raise RuntimeError(typ)
 return build(name,depth+1,True,seed)
def build(name,depth=0,present=True,seed=0):
 cons,params,typ=defs[name];flags={};b=u32(cons)
 for p in params:
  n,t=p.split(':',1)
  if t=='#':flags[n]=0
  else:
   m=re.match(r'(flags\d*)\.(\d+)\?(.+)',t)
   if m and present and depth<7:flags[m[1]]|=1<<int(m[2])
 for p in params:
  n,t=p.split(':',1)
  if t=='#':b+=u32(flags[n]);continue
  m=re.match(r'(flags\d*)\.(\d+)\?(.+)',t)
  if m:
   if not flags[m[1]]&(1<<int(m[2])):continue
   t=m[3]
  if t=='true':continue
  # Same IDs intentionally recur with distinct bytes: emplace keeps FIRST.
  if name in ['photo','document'] and n=='id':b+=u64(100 if name=='photo' else 200)
  elif name in ['photo','document'] and n=='file_reference':b+=string(bytes([seed%200+1,9]))
  else:b+=val(t,depth+1,seed)
 return b
roots=['Photo','Document','Page','RichMessage','WallPaper','Theme','WebPageAttribute','StarGift','StarGiftAttribute','WebPage','Game','MessageExtendedMedia','MessageMedia','MessageReplyHeader','DraftMessage','Dialog','Message','StoryItem','messages.Messages','messages.PeerDialogs','photos.Photos','users.UserFull','ChatFull','messages.ChatFull','ForumTopic','messages.ForumTopics','messages.RecentStickers','messages.FavedStickers','messages.StickerSet','messages.SavedGifs','SavedDialog','messages.SavedDialogs','account.SavedRingtones','help.PremiumPromo','messages.WebPage','stories.Stories','users.SavedMusic']
lines=[]
for typ in roots:
 for name in types[typ]:
  for present in [False,True]:lines.append('refs '+typ+' '+build(name,present=present).hex())
# Limits all types, unsigned >2GiB, boundaries including negative file sizes.
for s in range(3):
 for t in range(7):
  for limit in [-1,0,1,12*1024*1024,50*1024*1024,2147483648,MAX:=8000*512*1024]:
   for n in [-1,0,1,limit,limit+1,MAX]:lines.append(f'limit {s} {t} {limit} {n}')
for v in [1,2,3,0,4,255]:
 base=bytes([v])+b''.join(struct.pack('>i',x) for x in [-1,0,123,2147483647,-2147483648,-1000000000,-1]*3)
 for n in range(len(base)+1):lines.append('auto '+(base[:n].hex() or '-'))
for raw in [0,1,-1,2147483647,-2147483648,-100000000,-200000000]:
 base=b'\3'+struct.pack('>i',raw)*21
 for tail in [b'',struct.pack('>i',0),struct.pack('>i',-1),struct.pack('>i',1),struct.pack('>i',1)+struct.pack('>Qb',17|1<<55,1),struct.pack('>i',3)+struct.pack('>Qb',17|1<<55,1)+struct.pack('>Qb',17|1<<55,2)+struct.pack('>Qb',17|1<<55,0)]:lines.append('auto '+(base+tail).hex())
DAY=86400;WEEK=7*DAY
intervals=[[],[(0,DAY)],[(0,DAY-60)],[(0,WEEK)],[(0,WEEK+DAY)],[(WEEK-3600,WEEK+3600)],[(-3600,3600)],[(0,0),(10,0)],[(0,100),(100,300),(200,400)],[(0,3*DAY)],[(DAY-3600,DAY+6*3600)],[(DAY-3600,DAY+6*3600+1)]]
for day in range(7):
 for pairs in intervals:lines.append(f'business {day} {len(pairs)} '+' '.join(f'{a} {b}' for a,b in pairs))
for rating in [-1,0,0.0000005,1.5,999999.9999995,1000000,1000001]:
 for now,was,decay in [(0,0,1),(60,0,60),(-60,0,60),(1234,1000,2419200)]:lines.append(f'rating {rating} {now} {was} {decay}')
lines+=['rights '+str(n) for n in [0,1,2,3,16383,16384,0xffffffff]+[1<<b for b in range(14)]]
lines+=[f'hash {count} {value}' for count in [0,1,64,65] for value in [0,1,0xffffffffffffffe0]]
lines+=['audio','origin']
out.write_text('\n'.join(lines)+'\n')
print(len(lines),'input cases')
