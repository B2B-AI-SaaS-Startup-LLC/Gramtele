#include "pure.hpp"
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonArray>
#include <QtCore/QJsonObject>
#include <QtCore/QtEndian>
void Emit(const QJsonObject&o){std::cout<<QJsonDocument(o).toJson(QJsonDocument::Compact).constData()<<'\n';}
QString Hex(const auto &v){auto b=QByteArray();mtpBuffer words;words.push_back(v.type());v.write(words);for(auto w:words){w=qToLittleEndian(w);b.append(reinterpret_cast<const char*>(&w),4);}return b.toHex();}
int main(){
 for(uint32 bits:{0U,1U,4U,0x7fffffffU,0xffffffffU,0x60000000U}){
  auto f=UserDataFlags::from_raw(bits);QJsonArray a;
  for(bool b:{isVerified(f),isScam(f),isFake(f),isBotInlineGeo(f),isSupport(f),isInaccessible(f),applyMinPhoto(f),hasPersonalPhoto(f),hasStoriesHidden(f),hasRequirePremiumToWrite(f),hasStarsPerMessage(f),messageMoneyRestrictionsKnown(f),readDatesPrivate(f),allowsForwarding(f),isAyuNoForwards(f),hasActiveStories(f),hasUnreadStories(f),hasActiveVideoStream(f),storiesCorrespondent(f)})a.push_back(b);
  Emit({{"op","user_flags"},{"bits",double(bits)},{"result",a}});
 }
 for(int i=0;i<9;++i)for(bool boosts:{false,true})for(bool forbid:{false,true})for(int r:{1,1<<23,1<<25}){
  auto flags=ChannelDataFlags();switch(i){case 1:flags=ChannelDataFlag::Left;break;case 2:flags=ChannelDataFlag::Left|ChannelDataFlag::HasLink;break;case 3:flags=ChannelDataFlag::Left|ChannelDataFlag::HasLink|ChannelDataFlag::JoinToWrite;break;case 4:flags=ChannelDataFlag::Monoforum;break;case 5:flags=ChannelDataFlag::Broadcast;break;case 6:flags=ChannelDataFlag::Creator;break;case 7:flags=ChannelDataFlag::Forum;break;case 8:flags=ChannelDataFlag::MonoforumDisabled;break;}
  Emit({{"op","channel_send"},{"flags",double(flags.value())},{"boosts",boosts},{"forbid",forbid},{"rights",r},{"result",ChannelCanSend(flags,false,boosts,ChatRestrictions(),ChatRestrictions::from_raw(r),ChatRestrictions::from_raw(r),forbid)}});
 }
 for(int f:{0,1,2,4,8,16,32,64,128,256,512,1024})for(int admin:{0,128})Emit({{"op","chat_send"},{"flags",f},{"admin",admin},{"result",ChatCanSend(ChatDataFlags::from_raw(f),ChatAdminRights::from_raw(admin),ChatRestrictions::from_raw(1<<25),ChatRestrictions::from_raw(1<<25))}});
 for(int t:{0,1,2,3,4,59,60,3599,3600,43199,43200,86400})for(bool local:{false,true}){
  const auto now=TimeId(1700000000);const auto status=LastseenStatus::OnlineTill(now-t,local,true);
  Emit({{"op","timeout"},{"raw",double(status.serialize())},{"now",now},{"tomorrow",123},{"seconds",OnlinePhraseChangeInSeconds(status,now,123)},{"ms",double(OnlineChangeTimeout(status,now,123))}});
 }
 for(auto status:{MTP_userStatusEmpty(),MTP_userStatusOnline(MTP_int(1700000001)),MTP_userStatusOffline(MTP_int(1699999999)),MTP_userStatusRecently(MTP_flags(MTPDuserStatusRecently::Flag::f_by_me)),MTP_userStatusLastWeek(MTP_flags(MTPDuserStatusLastWeek::Flag::f_by_me)),MTP_userStatusLastMonth(MTP_flags(MTPDuserStatusLastMonth::Flag::f_by_me))})for(bool local:{false,true}){auto current=LastseenStatus::OnlineTill(1700000030,local,true);Emit({{"op","lastseen"},{"wire",Hex(status)},{"current",double(current.serialize())},{"result",double(LastseenFromMTP(status,current).serialize())}});}
 for(bool chatlist:{false,true})for(int rule:{0,1,31,255}){
  auto title=MTP_textWithEntities(MTP_string("Title"),MTP_vector<MTPMessageEntity>());auto pinned=MTP_vector<MTPInputPeer>({MTP_inputPeerChat(MTP_long(20)),MTP_inputPeerChat(MTP_long(10))});auto inc=MTP_vector<MTPInputPeer>({MTP_inputPeerChat(MTP_long(30))});
  auto v=chatlist?MTP_dialogFilterChatlist(MTP_flags(MTPDdialogFilterChatlist::Flag::f_emoticon|MTPDdialogFilterChatlist::Flag::f_color|MTPDdialogFilterChatlist::Flag::f_title_noanimate),MTP_int(7),title,MTP_string("🙂"),MTP_int(255),pinned,inc):MTP_dialogFilter(MTP_flags(MTPDdialogFilter::Flags::from_raw((rule&31)|((rule&224)<<6)|(1<<25)|(1<<27)|(1<<28))),MTP_int(7),title,MTP_string("🙂"),MTP_int(255),pinned,inc,MTP_vector<MTPInputPeer>({MTP_inputPeerChat(MTP_long(40))}));
  Emit({{"op","filter_tl"},{"chatlist",chatlist},{"rule",rule},{"wire",Hex(v)}});
 }
 UnreadState a;a.messages=10;a.chats=3;a.known=true;UnreadState b;b.messages=4;b.chats=2;b.known=false;auto c=a-b;Emit({{"op","unread"},{"messages",c.messages},{"chats",c.chats},{"known",c.known}});
}
