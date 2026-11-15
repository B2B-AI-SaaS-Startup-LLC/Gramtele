#include "stream-chains.hpp"
#include "extra-wire.hpp"
#include "document-current.hpp"
namespace Storage { namespace {
constexpr auto kArchiveEntryOverhead = int64(128);
#include "archive-pure.hpp"
#include "archive-writer.hpp"
}}
QJsonObject ReadDocumentHeader(const QByteArray&,int);
void Emit(QJsonObject value){if(value["op"]=="document"){const auto parsed=ReadDocumentHeader(QByteArray::fromHex(value["hex"].toString().toLatin1()),value["app"].toInt());for(auto i=parsed.begin();i!=parsed.end();++i)value[i.key()]=i.value();}std::cout<<QJsonDocument(value).toJson(QJsonDocument::Compact).constData()<<'\n';}
QByteArray Image(){return QByteArray();}
void WriteImage(QDataStream &s,QByteArray bytes=Image()){s<<qint32(INT32_MIN)<<bytes;}
QByteArray Peer(int kind,int version,int app,bool tagged,bool nulls){
 QByteArray bytes;QDataStream s(&bytes,QIODevice::WriteOnly);s.setVersion(QDataStream::Qt_5_1);
 const quint64 id=(quint64(0x80|kind)<<48)|123;
 s<<id;
 if(tagged)s<<quint64(0x77ffffffffffffffULL)<<qint32(version)<<quint64(0);
 else s<<quint64(0);
 WriteImage(s);if(version>0)s<<qint32(1);
 auto name=nulls?QString():QString::fromUtf8("Тест 😀");
 if(kind==0){
  if(app>=2008007 && version>=3)WriteUser(s,name,QStringLiteral(""),QStringLiteral("+123"),QStringLiteral("user"),99,17,QString(),0xc0000001,1,7,true);
  else {s<<name<<QStringLiteral("")<<QStringLiteral("+123")<<QStringLiteral("user")<<quint64(99);if(app>=9012)s<<qint32(17);if(app>=9016)s<<QString();s<<quint32(0xc0000001)<<qint32(1)<<qint32(7);if(version>2)s<<qint32(1);}
 } else if(kind==1)WriteChat(s,name,23,456,9,qint32(0x89abcdef),qint32(0x12345600),0x80000001,QString());
 else WriteChannel(s,name,99,456,0x80000201,QString());
 return bytes;
}
QByteArray Doc(int version,int app,int type,int setType,bool nulls){
 QByteArray bytes;QDataStream s(&bytes,QIODevice::WriteOnly);s.setVersion(QDataStream::Qt_5_1);
 const auto name=nulls?QString():QString::fromUtf8("файл 😀.bin");const auto mime=QStringLiteral("application/octet-stream");
 if(version==6){WriteDocumentCurrent(s,123,99,456,QByteArray(),name,mime,4,0xf0000001,320,240,type,type==3,QString::fromUtf8("🙂"),setType==3?2:(setType==4?1:0),setType?123:0,true,12345,false,true,true,QByteArray(),0,QByteArray(),0,false,QByteArray());return bytes;}
 else {s<<quint64(123)<<quint64(99)<<qint32(456);if(app>=1003013)s<<QByteArray();if(app>=9061){if(version>=0)s<<qint32(INT32_MAX)<<qint32(version);else s<<qint32(42);}s<<name<<mime<<qint32(4)<<qint32(0xf0000001)<<qint32(320)<<qint32(240)<<qint32(type);}
 if(type==3){s<<QString::fromUtf8("🙂")<<qint32(setType);if(version>=6)s<<qint64(12345)<<qint32(1)<<qint32(1);else if(version>=3){s<<qint32(12);if(version>=4)s<<qint32(1);if(version>=5)s<<qint32(1);}}
 else {if(version>=6)s<<qint64(12345);else s<<qint32(12);}
 WriteImage(s);
 if(version>=1){s<<qint32(0);WriteImage(s);s<<qint32(0);if(version>=2)s<<qint32(0)<<QByteArray();}
 return bytes;
}
int main(){
 {QMimeDatabase database;const auto mime=database.mimeTypeForName("application/octet-stream");QJsonArray globs;for(const auto &glob:mime.globPatterns())globs.push_back(glob);Emit({{"op","registry"},{"query","application/octet-stream"},{"name",mime.name()},{"globs",globs},{"filter",mime.filterString()}});}

 for(int kind=0;kind<=8;++kind)for(bool inMessage:{false,true}){
  QByteArray b;QDataStream stream(&b,QIODevice::WriteOnly);stream.setVersion(QDataStream::Qt_5_1);
  const quint64 volume=kind==7?0:55;
  WriteStorage(stream,4,kind|0x20|(inMessage?0x40:0),'m',inMessage?0x20000:7,99,123,volume,inMessage?42:0,8,QByteArray("ref",3));stream<<qint32(32)<<qint32(64);
  Emit({{"op","location"},{"kind",kind},{"in_message",inMessage},{"hex",QString(b.toHex())}});
 }

 for(int kind=0;kind!=3;++kind)for(bool nulls:{false,true}){
  for(int version=0;version<=3;++version){int app=2008007;const auto bytes=Peer(kind,version,app,true,nulls);Emit({{"op","peer"},{"kind",kind},{"version",version},{"app",app},{"nulls",nulls},{"hex",QString(bytes.toHex())},{"modern",QString(Peer(kind,3,app,true,nulls).toHex())}});}
  for(int app:{8000,9012,9016,1003013})Emit({{"op","peer"},{"kind",kind},{"version",0},{"app",app},{"nulls",nulls},{"hex",QString(Peer(kind,0,app,false,nulls).toHex())}});
 }
 for(int type=0;type!=8;++type)for(int version=-1;version<=6;++version)for(bool nulls:{false,true}){
  int app=2008007;Emit({{"op","document"},{"kind",type},{"version",version},{"app",app},{"nulls",nulls},{"hex",QString(Doc(version,app,type,3,nulls).toHex())}});
 }
 for(int app:{8000,9060,9061,1003012,1003013})for(int type:{0,3})Emit({{"op","document"},{"kind",type},{"version",-1},{"app",app},{"nulls",false},{"hex",QString(Doc(-1,app,type,1,false).toHex())}});
 for(const auto &name:{"a.zip","x.JPG",".zip","a/.zip","a/.x.zip","x.tXt","x.zst","dir/name.MP4","x.zip\n"})Emit({{"op","compressed"},{"name",name},{"value",Storage::AlreadyCompressedName(QString::fromUtf8(name))}});
 auto counts=base::flat_map<QString,int>();for(const auto &name:{"a.txt","A.TXT","a (2).txt","a.txt",".zip",".zip","x","X"})Emit({{"op","dedup"},{"name",name},{"value",Storage::DedupedName(counts,QString::fromUtf8(name))}});
 Storage::ArchiveEntries entries;entries.list.push_back({QString(),QString(),QDateTime(),100,false});entries.list.push_back({QString(),QString(),QDateTime(),0,true});Emit({{"op","estimate"},{"value",double(Storage::NativeArchiveSizeEstimate(entries))}});
 for(const auto &url:{"https://example.com/a","http://localhost/x","file:///tmp/a","ftp://host/a","HTTP://host","http:abc","https:///x","https://","/x","http://a b"}){QUrl u(QString::fromUtf8(url));Emit({{"op","url"},{"url",url},{"value",u.isValid()&&(u.scheme()=="http"||u.scheme()=="https")}});}
 {
  QDir().mkpath(QDir::tempPath());QFile file(QDir::tempPath()+"/input.txt");file.open(QIODevice::WriteOnly);QByteArray payload;for(int i=0;i<100;++i)payload.append("hello archive\n");file.write(payload);file.close();
  auto modified=QDateTime(QDate(2020,1,2),QTime(3,4,6));
  std::vector<Storage::ArchiveEntry> list{{file.fileName(),"root/input.txt",modified,payload.size(),false},{QString(),"root/empty/",modified,0,true}};
  Storage::WriteState state{.total=payload.size()};
  const auto written=Storage::WriteArchiveFile(list,state,1000000);
  QFile output(written.path);output.open(QIODevice::ReadOnly);Emit({{"op","zip"},{"hex",QString(output.readAll().toHex())}});output.close();QFile::remove(written.path);QFile::remove(file.fileName());
 }
 Emit({{"op","abi"},{"zip_info",int(sizeof(zip_fileinfo))},{"file_funcs",int(sizeof(zlib_filefunc64_def))}});
}
