#include "data/data_messages.h"
#include "data/data_sparse_ids.h"
#include "data/data_premium_limits.h"
#include "storage/storage_sparse_ids_list.h"
#include "business.cpp"
bool premium=false;int overrideValue=0;
extern "C" int config(const Data::PremiumLimits*,const QString &key,int fallback) asm("__wrap__ZNK4Data13PremiumLimits14appConfigLimitERK7QStringi");
extern "C" int config(const Data::PremiumLimits*,const QString &key,int fallback){return overrideValue ? overrideValue+key.size() : fallback;}
extern "C" bool paid(const Data::PremiumLimits*) asm("__wrap__ZNK4Data13PremiumLimits9isPremiumEv");
extern "C" bool paid(const Data::PremiumLimits*){return premium;}
extern "C" MTPInputBusinessRecipients recipients(const Data::BusinessRecipients&) asm("__wrap__ZN4Data16ForMessagesToMTPERKNS_18BusinessRecipientsE");
extern "C" MTPInputBusinessRecipients recipients(const Data::BusinessRecipients&){return MTP_inputBusinessRecipients(MTP_flags(0),MTP_vector<MTPInputUser>());}
using namespace Data;
void opt(std::optional<int> n){if(n)std::cout<<*n;else std::cout<<"?";std::cout<<",";}
MessagePosition pos(int n){return {FullMsgId(PeerId(9),MsgId(n)),n};}
void result(const MessagesResult &r){opt(r.count);opt(r.skippedBefore);opt(r.skippedAfter);for(auto p:r.messageIds)std::cout<<p.fullId.msg.bare<<";";std::cout<<"\n";}
void sparse(const SparseIdsSlice &r){opt(r.fullCount());opt(r.skippedBefore());opt(r.skippedAfter());for(int i=0;i<r.size();++i)std::cout<<r[i].bare<<";";std::cout<<"\n";}
int main(int argc,char **argv){QCoreApplication app(argc,argv);
 for(int mode=0;mode<4;++mode){premium=mode&1;overrideValue=(mode&2)?100:0;int token=0;PremiumLimits p(reinterpret_cast<Main::Session*>(&token));
 #define OUTPUT(name) std::cout<<"premium,"<<mode<<","<<#name<<","<<p.name()<<"\n";
 #include "getters.inc"
 #undef OUTPUT
 }
 for(int n=0;n<7;++n){MessagesList l;std::vector<MessagePosition> ids;for(int i=1;i<=n;++i)ids.push_back(pos(i*10));l.addSlice(std::move(ids),FullMessagesRange,n);
 for(int around=5;around<=75;around+=10)for(int b=0;b<3;++b)for(int a=0;a<3;++a){std::cout<<"list,"<<n<<","<<around<<","<<b<<","<<a<<",";result(l.snapshot({pos(around),b,a}));}
 l.removeOne(pos(999));std::cout<<"absent,"<<n<<",";result(l.snapshot({pos(20),10,10}));
 l.removeLessThan(pos(20));std::cout<<"less,"<<n<<",";result(l.snapshot({pos(20),10,10}));
 l.invalidateBottom();std::cout<<"bottom,"<<n<<",";result(l.snapshot({pos(20),10,10}));
 }
 for(int mode=0;mode<4;++mode){SparseIdsSliceBuilder b(MsgId(30),1,1);rpl::lifetime life;
 b.insufficientAround()|rpl::on_next([&](auto e){std::cout<<"request,"<<mode<<","<<e.aroundId.bare<<","<<int(e.direction)<<"\n";},life);
 b.checkInsufficient();Storage::SparseIdsListResult r{mode&1?std::optional(10):std::nullopt,mode&2?std::optional(0):std::nullopt,std::nullopt,{MsgId(10),MsgId(20),MsgId(30),MsgId(40),MsgId(50)}};
 b.applyInitial(r);std::cout<<"builder,"<<mode<<",initial,";sparse(b.snapshot());
 b.removeOne(MsgId(20));std::cout<<"builder,"<<mode<<",remove,";sparse(b.snapshot());
 b.invalidateBottom();std::cout<<"builder,"<<mode<<",bottom,";sparse(b.snapshot());
 b.removeAll();std::cout<<"builder,"<<mode<<",all,";sparse(b.snapshot());
 }
 for(int m=0;m<3;++m)for(int p=0;p<3;++p)for(int dir=0;dir<2;++dir){SparseIdsMergedSlice s({PeerId(9),0,PeerId(0),PeerId(8),MsgId(dir? -20:20)},SparseIdsSlice({MsgId(10),MsgId(20)},2,p?std::optional(p-1):std::nullopt,0),SparseIdsSlice({MsgId(ServerMaxMsgId-30),MsgId(ServerMaxMsgId-20)},2,0,m?std::optional(m-1):std::nullopt));
 std::cout<<"merged,"<<m<<","<<p<<","<<dir<<",";opt(s.fullCount());opt(s.skippedBefore());opt(s.skippedAfter());std::cout<<s.size()<<",";for(int i=0;i<s.size();++i){auto id=s[i];std::cout<<id.peer.value<<":"<<id.msg.bare<<";";}std::cout<<",";opt(s.indexOf(FullMsgId(PeerId(9),20)));auto id=s.nearest(MsgId(dir?-25:15));if(id)std::cout<<id->peer.value<<":"<<id->msg.bare;std::cout<<"\n";}
 for(int n=0;n<5;++n){WorkingHours h;h.timezoneId="Europe/Moscow";for(int i=0;i<n;++i)h.intervals.list.push_back({i*3600+17,i*3600+1801});mtpBuffer b;ToMTP(h).write(b);QByteArray bytes(reinterpret_cast<const char*>(b.data()),b.size()*4);std::cout<<"hours,"<<n<<","<<bytes.toHex().toStdString()<<"\n";}
 for(int t=1;t<4;++t){AwaySchedule s; s.type=AwayScheduleType(t);s.customInterval={-123,456};mtpBuffer b;ToMTP(s).write(b);std::cout<<"schedule,"<<t<<","<<QByteArray(reinterpret_cast<const char*>(b.data()),b.size()*4).toHex().toStdString()<<"\n";}
 for(int mode=0;mode<2;++mode){AwaySettings away;away.schedule.type=AwayScheduleType::Always;away.shortcutId=7;away.offlineOnly=mode;mtpBuffer b;ToMTP(away).write(b);std::cout<<"away,"<<mode<<","<<QByteArray(reinterpret_cast<const char*>(b.data()),b.size()*4).toHex().toStdString()<<"\n";}
 GreetingSettings greeting;greeting.shortcutId=8;greeting.noActivityDays=7;mtpBuffer b;ToMTP(greeting).write(b);std::cout<<"greeting,"<<QByteArray(reinterpret_cast<const char*>(b.data()),b.size()*4).toHex().toStdString()<<"\n";

}
