from pathlib import Path
import sys,re,json,hashlib
root,out,gen=map(Path,sys.argv[1:]);out.mkdir(parents=True,exist_ok=True)
def read(f):return (root/'SourceFiles'/f).read_text()
def body(s,sig):
 a=s.index('{',s.index(sig));i=a+1;d=1
 while d:
  if s[i]=='{':d+=1
  elif s[i]=='}':d-=1
  i+=1
 return s[a+1:i-1]
r=''
for file,name in [('data/data_user.h','UserDataFlag'),('data/data_chat.h','ChatDataFlag'),('data/data_channel.h','ChannelDataFlag')]:
 r+=re.search(r'enum class '+name+r'[^\{]*\{.*?\n\};',read(file),re.S)[0]+'\ninline constexpr bool is_flag_type('+name+'){return true;}\n'
r+='using UserDataFlags=base::flags<UserDataFlag>;using ChatDataFlags=base::flags<ChatDataFlag>;using ChannelDataFlags=base::flags<ChannelDataFlag>;\n'
r+='#include "data/data_chat_participant_status.h"\n#include "data/data_lastseen_status.h"\n'
s=read('data/data_user.cpp');r+='using namespace Data;\nLastseenStatus LastseenFromMTP(const MTPUserStatus &status,LastseenStatus currentStatus){'+body(s,'Data::LastseenStatus LastseenFromMTP(')+'}\n'
for name in ['isVerified','isScam','isFake','isBotInlineGeo','isSupport','isInaccessible','applyMinPhoto','hasPersonalPhoto','hasStoriesHidden','hasRequirePremiumToWrite','hasStarsPerMessage','messageMoneyRestrictionsKnown','readDatesPrivate','allowsForwarding','isAyuNoForwards','hasActiveStories','hasUnreadStories','hasActiveVideoStream','storiesCorrespondent']:
 r+='bool '+name+'(UserDataFlags bits){using Flag=UserDataFlag;'+body(s,'bool UserData::'+name+'()').replace('flags()','bits').replace('_flags.current()','bits')+'}\n'
s=read('data/data_peer_values.cpp')
a=s.rfind('ChannelDataFlags flags,',0,s.index('bool postMessagesRight,'));b=body(s[a:], 'ChannelDataFlags flags,')
r+='bool ChannelCanSend(ChannelDataFlags flags,bool postMessagesRight,bool unrestrictedByBoosts,ChatRestrictions sendRestriction,ChatRestrictions defaultSendRestriction,ChatRestrictions rights,bool forbidInForums){using Flag=ChannelDataFlag;'+b+'}\n'
a=s.index('ChatDataFlags flags,');b=body(s[a:], 'ChatDataFlags flags,')
# Preserve the actual lambda body while projecting its Change.value field only.
r+='bool ChatCanSend(ChatDataFlags flags,ChatAdminRights adminValue,ChatRestrictions defaultSendRestrictions,ChatRestrictions rights){'+b.replace('adminRights.value','adminValue')+'}\n'
# Only the local-calendar boundary is replaced by the caller-supplied duration.
b=body(s,'int OnlinePhraseChangeInSeconds(');a=b.index('\n\tconst auto nowFull');b=b[:a]+'\n return std::max(tomorrowSeconds,0);\n'
r+='int OnlinePhraseChangeInSeconds(LastseenStatus status,TimeId now,int tomorrowSeconds){'+b+'}\n'
r+='static constexpr auto kMinOnlineChangeTimeout=crl::time(1000);static constexpr auto kMaxOnlineChangeTimeout=86400*crl::time(1000);\n'
r+='crl::time OnlineChangeTimeout(LastseenStatus status,TimeId now,int tomorrowSeconds){'+body(s,'crl::time OnlineChangeTimeout(Data::LastseenStatus status').replace('OnlinePhraseChangeInSeconds(status, now)','OnlinePhraseChangeInSeconds(status, now, tomorrowSeconds)')+'}\n'
# Actual unread arithmetic from original header; no object graph substitutes.
u=read('dialogs/dialogs_common.h');a=u.index('struct UnreadState {');b=u.index('\n#ifdef _DEBUG',a);r+=u[a:b]+'\n'
(out/'pure.hpp').write_text(r)
# Link only genuine generated constructor/read/write bodies for selected TL values.
scheme=(gen/'scheme.cpp').read_text();names=['dialogFilter','dialogFilterChatlist','dialogFilterDefault','textWithEntities','inputPeerChat','inputPeerEmpty','inputPeerSelf','inputPeerUser','inputPeerChannel','inputPeerUserFromMessage','inputPeerChannelFromMessage','userStatusEmpty','userStatusOnline','userStatusOffline','userStatusRecently','userStatusLastWeek','userStatusLastMonth']
names+=re.findall(r'inline static MTP\w+ new_((?:messageEntity|inputMessageEntity|inputUser)\w*)\(',scheme)
r='#include "scheme.h"\nnamespace MTP::details {class TypeCreator final {public:\n'
for name in names:
 m=re.search(r'inline static MTP\w+ new_'+name+r'\([^\n]*\) \{',scheme);assert m,name
 r+=m.group(0)+body(scheme,m.group(0))+'}\n'
r+='};}\n'
for name in ['dialogFilter','textWithEntities','inputPeer','userStatus','messageEntity','inputUser']:
 start=scheme.index('MTP'+name+'::MTP'+name+'()');end=re.search(r'^MTP(?!D)\w+::MTP(?!D)\w+\(\)',scheme[start+1:],re.M);assert end
 r+=scheme[start:start+1+end.start()]
(out/'scheme_user.cpp').write_text(r)
files=[f'data/{n}.{e}' for n in ['data_user','data_peer_values','data_chat_filters','data_folder','data_unread_value','data_groups'] for e in ['h','cpp']]+['data/data_lastseen_status.h','data/data_chat.h','data/data_channel.h','data/data_chat_participant_status.h','dialogs/dialogs_common.h','mtproto/scheme/api.tl']
hashes={f:hashlib.sha256((root/'SourceFiles'/f).read_bytes()).hexdigest() for f in files}
for f in ['scheme.h','scheme.cpp']:hashes['generated/'+f]=hashlib.sha256((gen/f).read_bytes()).hexdigest()
(out/'source-sha256.json').write_text(json.dumps(hashes,indent=2)+'\n')
