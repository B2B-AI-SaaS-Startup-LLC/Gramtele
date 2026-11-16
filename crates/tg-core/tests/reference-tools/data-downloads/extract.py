from pathlib import Path
import sys,re,hashlib,json
root,out=map(Path,sys.argv[1:]);out.mkdir(parents=True,exist_ok=True)
def block(text,start):
 a=text.index(start);b=text.index('{',a);i=b+1;d=1
 while d:d+=(text[i]=='{')-(text[i]=='}');i+=1
 return text[a:i]
s=(root/'SourceFiles/data/data_msg_id.h').read_text()
(out/'ids.hpp').write_text('\n'.join(block(s,'struct '+name+' {')+';'for name in ['MsgId','FullMsgId','GlobalMsgId']))
s=(root/'SourceFiles/data/data_download_manager.cpp').read_text();a=s.index('Fn<std::optional<QByteArray>()> DownloadManager::serializator(');b=s.index('auto result = QByteArray();',a);e=s.index('return result;',b)+len('return result;');writer=s[b:e]
writer=re.sub(r'const auto &data = sessionData\(strong\);\s*','',writer).replace('data.downloaded','downloaded')
a=s.index('std::vector<DownloadedId> DownloadManager::deserialize(');b=s.index('if (serialized.isEmpty())',a);e=s.index('\nvoid DownloadManager::untrack(',b);reader=s[b:e].rstrip();assert reader.endswith('}');reader=reader[:-1]
r='#include "data/data_download_manager.h"\n#include "storage/serialize_common.h"\nnamespace Data{constexpr auto kMaxFileSize=4000*int64(1024*1024);\n'
r+='QByteArray OriginalWrite(const std::vector<DownloadedId>&downloaded){'+writer+'}\n'
r+='std::vector<DownloadedId> OriginalRead(const QByteArray&serialized){'+reader+'}\n}\n';(out/'codec.cpp').write_text(r)
files=['data/data_download_manager.h','data/data_download_manager.cpp','data/data_msg_id.h','data/data_peer_id.h','storage/serialize_common.h','iv/iv_rich_page.h']
(out/'source-sha256.json').write_text(json.dumps({f:hashlib.sha256((root/'SourceFiles'/f).read_bytes()).hexdigest()for f in files},indent=2)+'\n')
