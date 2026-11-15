#include "hash-body.hpp"
#include "upload-layout.hpp"
void Emit(const QJsonObject &object){std::cout<<QJsonDocument(object).toJson(QJsonDocument::Compact).constData()<<'\n';}
int main(){
 for(int n:{0,1,15,16,1024,131071,131072}){
  QByteArray data(n,Qt::Uninitialized);for(int i=0;i<n;++i)data[i]=char((i*17+3)&255);
  const auto span=bytes::make_span(data);auto digest=openssl::Sha256(span);QByteArray hash(reinterpret_cast<const char*>(digest.data()),digest.size());
  for(int limit:{0,1,131072})for(int mode:{0,1,2}){
   auto hashes=base::flat_map<int64,CdnFileHash>();if(mode!=0){auto saved=hash;if(mode==2)saved[0]=char(saved[0]^1);hashes.emplace(131072,CdnFileHash{limit,saved});}
   Emit({{"op","hash"},{"size",n},{"limit",limit},{"mode",mode},{"sha256",QString(hash.toHex())},{"result",int(Check(hashes,131072,span))}});
  }
 }
 for(int64 size:{0LL,1LL,1024LL,1048575LL,1048576LL,33554432LL,33554433LL,524288000LL,1048576000LL,2147483648LL,4294967296LL,8589934592LL,1099511627776LL}){Layout layout;layout.setDocSize(size);Emit({{"op","layout"},{"size",QString::number(size)},{"part",layout.docPartSize},{"count",layout.docPartsCount}});}
}
