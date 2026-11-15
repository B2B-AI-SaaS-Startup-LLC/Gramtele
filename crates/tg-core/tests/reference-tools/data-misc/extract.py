"""Unchanged pure bodies and declarations only; no fake owner/model classes."""
from pathlib import Path
import sys,re,hashlib,json
root,out=map(Path,sys.argv[1:3]);out.mkdir(exist_ok=True,parents=True)
files={}
def read(name):
 p=root/'SourceFiles'/name;s=p.read_text();files[name]=hashlib.sha256(p.read_bytes()).hexdigest();return s
def function(s,start):
 a=s.index(start);return s[a:s.index('\n}',a)+2]+'\n'
# No substitute PeerData/DocumentData implementations: drop all dereferencing
# helpers/Should overloads, retain the actual Full, Set and Single code verbatim.
s=read('data/data_auto_download.cpp');a=s.index('namespace Data {');b=s.index('Source SourceFromPeer');pure=s[a:b]+'}\n'
a=s.index('void SetDefaultsForSource');b=s.index('bool Should(',a);pure+=s[a:b]+'}\n}\n'
pure+='#include "data/data_audio_msg_id.h"\n'
audio=read('data/data_audio_msg_id.cpp')
for start in ['AudioMsgId::AudioMsgId()','uint32 AudioMsgId::CreateExternalPlayId()', 'AudioMsgId AudioMsgId::ForVideo()', 'AudioMsgId::Type AudioMsgId::type()', 'DocumentData *AudioMsgId::audio()', 'FullMsgId AudioMsgId::contextId()', 'uint32 AudioMsgId::externalPlayId()', 'bool AudioMsgId::changeablePlaybackSpeed()', 'AudioMsgId::operator bool()', 'bool AudioMsgId::operator<(', 'bool AudioMsgId::operator==(', 'bool AudioMsgId::operator!=(']:pure+=function(audio,start)
# Full unchanged FileOrigin declarations + all accumulator/GetFileReferences
# bodies. Only the header's unrelated data_types.h include is replaced by the
# real scalar aliases/FullMsgId/FullStoryId in prefix.hpp.
h=read('data/data_file_origin.h');h=h[h.index('namespace Data {'):];(out/'origin.hpp').write_text('#pragma once\n'+h)
s=read('data/data_file_origin.cpp');(out/'origin.cpp').write_text('#include "origin.hpp"\n'+s[s.index('namespace Data {'):])
# Actual business header is lightweight (forward declarations only).
pure+='#include "data/business/data_business_common.h"\nnamespace Data {\nnamespace {\nconstexpr auto kDay=WorkingInterval::kDay;constexpr auto kWeek=WorkingInterval::kWeek;constexpr auto kInNextDayMax=WorkingInterval::kInNextDayMax;\n'
s=read('data/business/data_business_common.cpp')
for name in ['[[nodiscard]] WorkingIntervals SortAndMerge(', '[[nodiscard]] WorkingIntervals MoveTailToFront(']:pure+=function(s,name)
pure+='}\n'+function(s,'BusinessRecipients BusinessRecipients::MakeValid(')
for name in ['WorkingIntervals WorkingIntervals::normalized()', 'WorkingIntervals ExtractDayIntervals(', 'bool IsFullOpen(', 'WorkingIntervals RemoveDayIntervals(', 'WorkingIntervals ReplaceDayIntervals(']:pure+=function(s,name)
for name in ['ChatbotsPermissions FromMTP(', 'MTPBusinessBotRights ToMTP(']:pure+=function(s,name)
pure+='}\n'
# Source rating and hash pure helpers; service/session code not substituted.
s=read('data/components/top_peers.cpp')
for name in ['[[nodiscard]] float64 RatingDelta(', '[[nodiscard]] quint64 SerializeRating(', '[[nodiscard]] float64 DeserializeRating(']:pure+=function(s,name)
pure+='#include "api/api_hash.h"\n'
(out/'pure.cpp').write_text('#include "data/data_auto_download.h"\n'+pure)
for name in ['data/data_auto_download.h','data/data_audio_msg_id.h','data/data_peer_id.h','data/data_peer_id.cpp','data/data_msg_id.h','data/data_types.h','data/business/data_business_common.h','data/data_location.h','api/api_hash.h','mtproto/scheme/api.tl']:read(name)
(out/'source-sha256.json').write_text(json.dumps(files,sort_keys=True,indent=2)+'\n')
