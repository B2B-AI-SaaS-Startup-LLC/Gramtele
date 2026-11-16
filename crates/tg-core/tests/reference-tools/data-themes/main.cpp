#include "data/data_wall_paper.h"
#include "gift.hpp"
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QJsonArray>
#include <QtCore/QDataStream>
#include <QtCore/QtEndian>
using namespace Data;
void Emit(const QJsonObject&o){std::cout<<QJsonDocument(o).toJson(QJsonDocument::Compact).constData()<<'\n';}
QString TL(const auto&v){mtpBuffer w;v.write(w);QByteArray b;for(auto i:w){i=qToLittleEndian(i);b.append(reinterpret_cast<const char*>(&i),4);}return b.toHex();}
void Dump(QString op,const QByteArray&bytes,const std::optional<WallPaper>&p){QJsonObject o{{"op",op},{"bytes",QString(bytes.toHex())},{"ok",bool(p)}};if(p){QJsonArray c;for(auto color:p->backgroundColors())c.push_back(color.name());o.insert("canonical",QString(p->serialize().toHex()));o.insert("key",p->key());o.insert("id",QString::number(p->id()));o.insert("owner",QString::number(p->ownerId().bare));o.insert("colors",c);o.insert("rotation",p->gradientRotation());o.insert("intensity",p->patternIntensity());o.insert("blurred",p->isBlurred());o.insert("null",p->isNull());o.insert("settings",TL(p->mtpSettings()));}Emit(o);}
int main(int argc,char**argv){
 if(argc>1){std::string line;while(std::getline(std::cin,line)){auto b=QByteArray::fromHex(QByteArray::fromStdString(line));Dump("reverse",b,WallPaper::FromSerialized(b));}return 0;}
 for(int rarity:{-9,-4,-3,-2,-1,0,1,100,1000,999999}){UniqueGiftAttribute a;a.rarityValue=rarity;QJsonObject o{{"op","rarity"},{"value",rarity},{"type",int(a.rarityType())},{"permille",a.rarityPermille()},{"special",UniqueGiftAttributeHasSpecialRarity(a)}};if(rarity>=-4&&rarity<0){auto c=UniqueGiftRarityBadgeColors(a.rarityType());o.insert("bg",QString::number((uint32(c.bg.red())<<24)|(uint32(c.bg.green())<<16)|(uint32(c.bg.blue())<<8)|c.bg.alpha()));o.insert("fg",QString::number((uint32(c.fg.red())<<24)|(uint32(c.fg.green())<<16)|(uint32(c.fg.blue())<<8)|c.fg.alpha()));}Emit(o);}
 for(int type=0;type!=3;++type)for(uint64 value:{0ULL,1ULL,0xffffffffULL,0x123456789abcdef0ULL}){auto id=GiftAttributeId{value,GiftAttributeIdType(type)};auto tl=AttributeToTL(id);auto back=FromTL(tl);Emit({{"op","attribute"},{"kind",type},{"value",QString::number(value)},{"wire",TL(tl)},{"back",QString::number(back.value)}});}
 int marker=0;auto peer=reinterpret_cast<PeerData*>(&marker);for(int64 n:{0LL,1LL,-1LL,0x123456789LL})for(bool chat:{false,true}){auto id=chat?SavedStarGiftId::Chat(peer,uint64(n)):SavedStarGiftId::User(MsgId(n));Emit({{"op","saved_id"},{"n",QString::number(n)},{"chat",chat},{"user",id.isUser()},{"present",bool(id)},{"message",QString::number(id.userMessageId().bare)},{"saved",QString::number(id.chatSavedId())}});}
 for(int id:{-999,-666,-665,-664,-2,-1,0,105,200})for(int flags:{0,1,2,8,16,31})for(auto slug:{QString(),QString(""),QString("Ab12Ef"),QString("aabbcc-ddeeff"),QString::fromUtf8("фото🙂")}){
  auto p=*WallPaper::FromLegacySerialized(uint64(id),123,flags,slug);p=p.withBlurred(flags&1).withPatternIntensity(flags==31?-100:50).withGradientRotation(123);Dump("current",p.serialize(),WallPaper::FromSerialized(p.serialize()));
 }
 for(int version:{-1,0,1,999})for(int count:{-1,0,1,2,3,4,5})for(int intensity:{-101,-100,0,100,101}){
  QByteArray b;QDataStream w(&b,QIODevice::WriteOnly);w.setVersion(QDataStream::Qt_5_1);w<<quint64(88)<<quint64(99)<<qint32(0x7fffffff)<<qint32(version)<<qint32(1)<<QString("slug")<<qint32(2)<<qint32(count);for(int i=0;i<std::max(count,0);++i)w<<quint32(i?0x112233:0xffffffff);w<<qint32(intensity)<<qint32(-45)<<quint64(0x8123456789abcdefULL);Dump("read",b,WallPaper::FromSerialized(b));
 }
 for(int flags:{0,1,2,8,16,31})for(int ownerWords:{0,1,2})for(quint32 color:{0xffffffffU,0U,0xabcdefU}){
  QByteArray b;QDataStream w(&b,QIODevice::WriteOnly);w.setVersion(QDataStream::Qt_5_1);w<<quint64(5)<<quint64(6)<<qint32(flags)<<QString("abc")<<qint32(2)<<quint32(color)<<qint32(100);if(ownerWords>0)w<<qint32(0x89abcdef);if(ownerWords>1)w<<qint32(0x81234567);Dump("legacy",b,WallPaper::FromSerialized(b));
 }
 for(auto slug:{"ff00Aa","112233-445566","112233~445566~778899","112233-445566-778899","bad"}){
  auto p=WallPaper::FromColorsSlug(slug);Dump("slug",p?p->serialize():QByteArray(),p);Emit({{"op","slug_input"},{"slug",slug},{"ok",bool(p)}});
 }
 auto d=DefaultWallPaper();Dump("default",d.serialize(),d);auto e=WallPaper::FromEmojiId("🙂");Dump("emoji",e.serialize(),e);
 auto good=d.serialize();for(int i=0;i<good.size();++i)Dump("truncated",good.first(i),WallPaper::FromSerialized(good.first(i)));Dump("trailing",good+"garbage",WallPaper::FromSerialized(good+"garbage"));
 for(auto value:{"-100","100","101","-101","+25"," 45 ","45x","2147483648",""}){
  auto p=DefaultWallPaper().withUrlParams({{"intensity",value},{"rotation",value},{"mode","foo+blur"},{"bg_color","112233-445566"}});QJsonObject o{{"op","params"},{"input",value},{"canonical",QString(p.serialize().toHex())},{"key",p.key()}};Emit(o);
 }
}
