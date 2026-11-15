from pathlib import Path
import sys,hashlib,json
root,out=map(Path,sys.argv[1:3]);out.mkdir(parents=True,exist_ok=True)
hashes={}
def read(name):
 p=root/'SourceFiles'/name;hashes[name]=hashlib.sha256(p.read_bytes()).hexdigest();return p.read_text()
def fun(s,name):
 a=s.index(name);return s[a:s.index('\n}',a)+2]+'\n'
s=read('data/components/passkeys.cpp')
code='#include "data/components/passkeys.h"\n#include "platform/platform_webauthn.h"\n#include "data/data_messages.h"\n#include "storage/storage_shared_media.h"\nnamespace Data {\n'+fun(s,'[[nodiscard]] PasskeyEntry FromTL(')+'}\n'
a=s.index('const auto credentialIdBase64 =');b=s.index('\n\t_session->api().request(MTPaccount_RegisterPasskey(',a)
a2=b+len('\n\t_session->api().request(');b2=s.index('\n\t)).done',a2)+3
code+='MTPaccount_RegisterPasskey RegisterCredential(const Platform::WebAuthn::RegisterResult &result) {\n'+s[a:b]+'\nreturn '+s[a2:b2]+';\n}\n'
a=s.index('const auto userHandleStr =');b=s.index('\n\tconst auto credentialIdBase64',a)
code+='void UserDc(const Platform::WebAuthn::LoginResult &result, Fn<void(int)> done, Fn<void(QString)> fail) {\n'+s[a:b]+'\ndone(userDc);\n}\n'
a=s.index('const auto credentialIdBase64',b);b=s.index('\n\tauto fromAuthKeyId',a)
code+='MTPInputPasskeyCredential LoginCredential(const Platform::WebAuthn::LoginResult &result) {\nconst auto userHandleStr = QString::fromUtf8(result.userHandle);\n'+s[a:b]+'\nreturn credential;\n}\n'
s=read('data/data_search_controller.cpp')
code+='namespace Api {\n'+fun(s,'MTPMessagesFilter PrepareSearchFilter(')+'}\n'
a=s.index('\tconst auto minId =',s.index('std::optional<SearchRequest> PrepareSearchRequest('));b=s.index('\n\tusing Flag =',a)
code+='QJsonArray SearchOffsets(MsgId messageId, Data::LoadDirection direction) {\n'+''.join(l+'\n' for l in s.splitlines() if l.startswith(('constexpr auto kSharedMediaLimit =','constexpr auto kFirstSharedMediaLimit =')))+s[a:b]+'\nreturn {mtpOffsetId,addOffset,limit};\n}\n'
a=s.index('\tconst auto minId =',s.index('HistoryRequest PrepareHistoryRequest('));b=s.index('\n\treturn MTPmessages_GetHistory(',a)
code+='QJsonArray HistoryOffsets(MsgId messageId, Data::LoadDirection direction) {\n'+''.join(l+'\n' for l in s.splitlines() if l.startswith('constexpr auto kHistoryLimit ='))+s[a:b]+'\nreturn {mtpOffsetId,addOffset,limit};\n}\n'
s=read('data/data_reply_preview.cpp');a=s.index('\tint w = image->width(), h = image->height();');b=s.index('\n\toptions |=',a)
fragment=s[a:b].replace('int w = image->width(), h = image->height();','').replace('st::historyReplyPreview','size').replace('style::DevicePixelRatio()','ratio')
code+='QJsonArray Geometry(int w,int h,int size,int ratio) {\n'+fragment+'\nreturn {thumbSize.width(),thumbSize.height(),size};\n}\n'
(out/'pure.cpp').write_text(code)
for stem in ['data/components/passkeys','data/components/recent_peers','data/data_reply_preview','data/data_streaming','data/data_search_controller','data/data_passkey_deserialize']:
 for ext in ['h','cpp']:read(stem+'.'+ext)
for name in ['platform/platform_webauthn.h','storage/storage_shared_media.h','data/data_types.h','mtproto/scheme/api.tl']:read(name)
(out/'source-sha256.json').write_text(json.dumps(hashes,sort_keys=True,indent=2)+'\n')
