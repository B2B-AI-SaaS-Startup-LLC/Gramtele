#include "base/basic_types.h"
#include <range/v3/all.hpp>
#include <QtCore/QByteArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QJsonArray>
#include <iostream>
#include "md5.hpp"
#include "parts.hpp"
#include "rules.hpp"
QString Hash(const QByteArray &b){QByteArray out(32,Qt::Uninitialized);hashMd5Hex(b.constData(),b.size(),out.data());int32 raw[4];hashMd5(b.constData(),b.size(),raw);QByteArray check(32,Qt::Uninitialized);hashMd5Hex(raw,check.data());if(check!=out)std::abort();return QString(out);}
void Emit(const QJsonObject &o){std::cout<<QJsonDocument(o).toJson(QJsonDocument::Compact).constData()<<'\n';}
int main(){
 for(int n:{0,1,15,55,56,63,64,65,32767,32768,32769,65536,65537,100000}){
  QByteArray data(n,Qt::Uninitialized);for(int i=0;i<n;++i)data[i]=char((17*i+3)&255);
  Parts p;for(int op:{0,1,2,3}){
   if(op==0){p.setFileData(data);p.setThumbData(data);}else if(op==1){p.setFileData({});p.setThumbData({});}else if(op==2){p.setFileData("abc");p.setThumbData("abc");}else{p.setFileData(data);p.setThumbData(data);}
   QJsonArray parts,thumbs;for(const auto &b:p.fileparts)parts.push_back(QJsonObject{{"size",b.size()},{"md5",Hash(b)}});for(const auto &b:p.thumbparts)thumbs.push_back(QJsonObject{{"size",b.size()},{"md5",Hash(b)}});
   Emit({{"op","parts"},{"size",n},{"step",op},{"fileparts",parts},{"thumbparts",thumbs},{"md5",QString(p.filemd5)},{"thumbmd5",QString(p.thumbmd5)},{"partssize",p.partssize},{"thumbbytes_md5",Hash(p.thumbbytes)},{"thumbbytes_size",p.thumbbytes.size()}});
  }
 }
 for(const auto &mime:{"image/jpeg","IMAGE/JPEG","image/png","image/gif","image/webp","video/mp4","video/webm","application/x-tgsticker",""})for(int64 size:{0LL,5242880LL,5242881LL})Emit({{"op","thumb"},{"mime",mime},{"size",size},{"required",FileThumbnailUploadRequired(mime,size)}});
 for(auto name:{"name","name.PNG",".file","a.b.c","dir/a.b/file","a.","фото.png"})Emit({{"op","mp4"},{"name",name},{"result",Mp4FileName(QString::fromUtf8(name))}});
 for(int w:{-1,0,1,20,21,320,1280,2560})for(int h:{0,1,20,21,320,1280})Emit({{"op","dimensions"},{"w",w},{"h",h},{"valid",ValidateThumbDimensions(w,h)}});
 for(int type=0;type!=5;++type)for(bool group:{false,true})for(bool photos:{false,true})Emit({{"op","group"},{"type",type},{"group",group},{"photos",photos},{"result",int(GroupTypeForFile(FileType(type),group,photos))}});
 for(bool large:{false,true})Emit({{"op","side"},{"large",large},{"result",PhotoSideLimit(large)}});
}
