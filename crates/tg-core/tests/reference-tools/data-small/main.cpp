#include "data/data_passkey_deserialize.h"
#include "pure.cpp"
QJsonValue hex(QByteArray value){return QString::fromLatin1(value.toHex());}
template<typename T> QJsonValue wire(const T &value){auto v=QVector<mtpPrime>();value.write(v);return hex(QByteArray(reinterpret_cast<const char*>(v.data()),v.size()*sizeof(mtpPrime)));}
void output(QJsonObject value){std::cout<<QJsonDocument(value).toJson(QJsonDocument::Compact).toStdString()<<"\n";}
int main(int argc,char**argv){QCoreApplication app(argc,argv);
 for(int flags=0;flags<4;++flags){auto data=MTP_passkey(MTP_flags(MTPDpasskey::Flag(flags)),MTP_string("id"),MTP_string("name"),MTP_int(-3),MTP_long(65535),MTP_int(9));auto entry=Data::FromTL(data.data());output({{"kind","entry"},{"flags",flags},{"value",QJsonArray{entry.id,entry.name,entry.date,int(entry.softwareEmojiId),entry.lastUsageDate}}});}
 for(int n=0;n<9;++n){auto bytes=QByteArray();for(int i=0;i<n;++i)bytes.push_back(char(250+i));auto reg=Platform::WebAuthn::RegisterResult{.credentialId=bytes,.attestationObject=QByteArray("a\0b",3),.clientDataJSON="{}"};auto login=Platform::WebAuthn::LoginResult{.clientDataJSON="{}",.credentialId=bytes,.authenticatorData="auth",.signature="sig",.userHandle="2:user"};output({{"kind","credential"},{"n",n},{"register",wire(RegisterCredential(reg))},{"login",wire(LoginCredential(login))},{"create",QString::fromStdString(Data::Passkey::SerializeClientDataCreate(bytes))},{"get",QString::fromStdString(Data::Passkey::SerializeClientDataGet(bytes))}});}
 const auto handles=QStringList{"1:","  +2 :user","0:user","-1:user","9999:x","10000:x","1","1:x:y",":x","01:x","2147483648:x","1.0:x",QString::fromUtf8("\xE3\x80\x80" "3" "\xE3\x80\x80:x"),QString::fromUtf8("\xD9\xA2:x")};
 for(const auto &h:handles){int dc=0;UserDc(Platform::WebAuthn::LoginResult{.userHandle=h.toUtf8()},[&](int d){dc=d;},[&](QString){});output({{"kind","dc"},{"handle",h},{"value",dc}});}
 for(int t=0;t<14;++t){auto filter=Api::PrepareSearchFilter(Storage::SharedMediaType(t));output({{"kind","filter"},{"type",t},{"wire",wire(filter)}});}
 for(auto id:{int64(-1),int64(0),int64(1),int64(0x3fffffff),int64(0x40000000),int64(1)<<56})for(int dir=0;dir<3;++dir){auto d=Data::LoadDirection(dir);output({{"kind","offset"},{"id",QString::number(id)},{"direction",dir},{"search",SearchOffsets(MsgId(id),d)},{"history",HistoryOffsets(MsgId(id),d)}});}
 for(int w:{-2,0,1,10,99})for(int h:{-2,0,1,10,99})for(int ratio:{1,2})output({{"kind","geometry"},{"w",w},{"h",h},{"ratio",ratio},{"value",Geometry(w,h,35,ratio)}});
 std::string input;int index=0;while(std::getline(std::cin,input)){auto bytes=QByteArray::fromStdString(input);auto r=Data::Passkey::DeserializeRegisterData(bytes);auto l=Data::Passkey::DeserializeLoginData(bytes);auto reg=QJsonValue();auto login=QJsonValue();
 if(r){QJsonArray params;for(auto p:r->pubKeyCredParams)params.push_back(QJsonObject{{"type",p.type},{"alg",p.alg}});reg=QJsonObject{{"rp_id",r->rp.id},{"rp_name",r->rp.name},{"user_id",hex(r->user.id)},{"user_name",r->user.name},{"display_name",r->user.displayName},{"challenge",hex(r->challenge)},{"params",params},{"timeout",r->timeout}};}
 if(l){QJsonArray creds;for(auto c:l->allowCredentials)creds.push_back(QJsonObject{{"id",hex(c.id)},{"type",c.type}});login=QJsonObject{{"challenge",hex(l->challenge)},{"rp_id",l->rpId},{"verify",l->userVerification},{"timeout",l->timeout},{"credentials",creds}};}
 output({{"kind","json"},{"index",index++},{"register",reg},{"login",login}});
 }
}
