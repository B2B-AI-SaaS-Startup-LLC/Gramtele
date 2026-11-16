from pathlib import Path
import sys,re,hashlib,json
root,out,gen=map(Path,sys.argv[1:]);out.mkdir(parents=True,exist_ok=True)
s=(root/'SourceFiles/data/data_wall_paper.cpp').read_text()
# Keep complete original pure function bodies and the genuine WallPaper class.
r='#include "data/data_wall_paper.h"\n#include "storage/serialize_common.h"\n#include "ui/color_int_conversion.h"\n'
utils=(root/'SourceFiles/core/utils.h').read_text();a=utils.index('template <typename Value, typename From, typename Till>');b=utils.index('\n}',a)+2
r+='namespace base{'+utils[a:b]+'}\n'
r+=s[s.index('namespace Ui {'):s.index('WallPaper::WallPaper(')]
pat=re.compile(r'(?m)^(?:(?:[\w:<>&* ]+?)\s+)?((?:\w+::)*\w+)\(([^;{}]*?)\)(?: const)?\s*\{')
excluded={'setLocalImageAsThumbnail','localThumbnail','isLocal','shareUrl','loadDocumentThumbnail','loadDocument','fileOrigin','mtpInput','GenerateDitheredGradient'}
for m in pat.finditer(s,s.index('WallPaper::WallPaper(')):
 name=m.group(1).split('::')[-1]
 if name in excluded:continue
 if name=='Create' and 'Main::Session' in m.group(2):continue
 a=m.start();brace=s.index('{',m.start());depth=1;i=brace+1
 while depth:
  depth+=(s[i]=='{')-(s[i]=='}');i+=1
 text=s[a:i]
 # The ctor's initializer is not matched by this regex; added below.
 if name=='WallPaper':continue
 if 'details::' in m.group(1):pass
 if name in ['UninitializedWallPaper','IsUninitializedWallPaper','TestingThemeWallPaper','IsTestingThemeWallPaper','TestingDefaultWallPaper','IsTestingDefaultWallPaper','TestingEditorWallPaper','IsTestingEditorWallPaper']:
  text=text.replace(name+'(', 'details::'+name+'(',1)
 r+=text+'\n'
r+='WallPaper::WallPaper(WallPaperId id):_id(id){}\n}\n'
(out/'paper.cpp').write_text(r)
def block(text,signature):
 a=text.index(signature);b=text.index('{',a);i=b+1;d=1
 while d:d+=(text[i]=='{')-(text[i]=='}');i+=1
 return text[a:i]
h=(root/'SourceFiles/data/data_star_gift.h').read_text();c=(root/'SourceFiles/data/data_star_gift.cpp').read_text()
m=(root/'SourceFiles/data/data_msg_id.h').read_text()
r=block(m,'struct MsgId {')+';\nclass PeerData;\n'
r+='namespace Data{\n'
for sig in ['enum class UniqueGiftRarity :','struct UniqueGiftAttribute {','struct UniqueGiftRarityColors {','enum class GiftAttributeIdType {','struct GiftAttributeId {','class SavedStarGiftId {']:r+=block(h,sig)+';\n'
for sig in ['[[nodiscard]] MTPStarGiftAttributeId AttributeToTL(', '[[nodiscard]] GiftAttributeId FromTL(', 'UniqueGiftRarityColors UniqueGiftRarityBadgeColors(', 'bool UniqueGiftAttributeHasSpecialRarity(']:r+=block(c,sig)+'\n'
r+='}\n';(out/'gift.hpp').write_text(r)
scheme=(gen/'scheme.cpp').read_text();names=['wallPaperSettings','starGiftAttributeIdBackdrop','starGiftAttributeIdModel','starGiftAttributeIdPattern'];r='#include "scheme.h"\nnamespace MTP::details{class TypeCreator final{public:\n'
for name in names:
 m=re.search(r'inline static MTP\w+ new_'+name+r'\([^\n]*\) \{',scheme);assert m
 i=m.end();d=1
 while d:d+=(scheme[i]=='{')-(scheme[i]=='}');i+=1
 r+=scheme[m.start():i]+'\n'
r+='};}\n'
for name in ['wallPaperSettings','starGiftAttributeId']:
 a=scheme.index('MTP'+name+'::MTP'+name+'()');b=re.search(r'^MTP(?!D)\w+::MTP(?!D)\w+\(\)',scheme[a+1:],re.M);assert b
 r+=scheme[a:a+1+b.start()]
(out/'scheme_paper.cpp').write_text(r)
files=['data/data_wall_paper.h','data/data_wall_paper.cpp','storage/serialize_common.h','ui/color_int_conversion.h','ui/color_int_conversion.cpp','mtproto/scheme/api.tl','core/utils.h','data/data_types.h','data/data_star_gift.h','data/data_star_gift.cpp','data/data_msg_id.h']
hashes={f:hashlib.sha256((root/'SourceFiles'/f).read_bytes()).hexdigest()for f in files}
for f in ['scheme.h','scheme.cpp']:hashes['generated/'+f]=hashlib.sha256((gen/f).read_bytes()).hexdigest()
(out/'source-sha256.json').write_text(json.dumps(hashes,indent=2)+'\n')
