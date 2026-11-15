"""Unchanged source bodies; owner/config calls are explicitly test seams, not fake models."""
from pathlib import Path
import re,sys,json,hashlib
root,out=map(Path,sys.argv[1:3]);out.mkdir(exist_ok=True,parents=True)
hashes={}
def read(n):
 p=root/'SourceFiles'/n;hashes[n]=hashlib.sha256(p.read_bytes()).hexdigest();return p.read_text()
def fun(s,start):
 a=s.index(start);return s[a:s.index('\n}',a)+2]+'\n'
s=read('data/data_premium_limits.cpp')
# Constructors and getters use only opaque Session pointer, never dereferenced.
a=s.index('PremiumLimits::PremiumLimits(');b=s.index('int PremiumLimits::appConfigLimit(')
premium=s[a:b];a=premium.index('int PremiumLimits::maxBoostLevel()');b=premium.index('\n}',a)+2
premium=premium[:a]+premium[b:]
(out/'premium.cpp').write_text('#include "data/data_premium_limits.h"\nnamespace Data {\n'+premium+'}\n')
s=read('data/business/data_business_common.cpp')
pure='#include "data/business/data_business_common.h"\nnamespace Data {\nnamespace {\nconstexpr auto kDay=WorkingInterval::kDay;constexpr auto kWeek=WorkingInterval::kWeek;constexpr auto kInNextDayMax=WorkingInterval::kInNextDayMax;\n'
for n in ['[[nodiscard]] WorkingIntervals SortAndMerge(', '[[nodiscard]] WorkingIntervals MoveTailToFront(']:pure+=fun(s,n)
pure+='}\n'+fun(s,'WorkingIntervals WorkingIntervals::normalized()')
s=read('data/business/data_business_info.cpp')
for n in ['[[nodiscard]] MTPBusinessWorkHours ToMTP(', '[[nodiscard]] MTPBusinessAwayMessageSchedule ToMTP(', '[[nodiscard]] MTPInputBusinessAwayMessage ToMTP(', '[[nodiscard]] MTPInputBusinessGreetingMessage ToMTP(']:pure+=fun(s,n)
pure+='}\n';(out/'business.cpp').write_text(pure)
for n in ['data/data_messages.h','data/data_messages.cpp','data/data_sparse_ids.h','data/data_sparse_ids.cpp','data/data_abstract_sparse_ids.h','data/data_premium_limits.h','data/business/data_business_common.h','data/business/data_business_info.h','data/data_peer_id.h','data/data_msg_id.h','storage/storage_sparse_ids_list.h','mtproto/scheme/api.tl']:read(n)
(out/'source-sha256.json').write_text(json.dumps(hashes,sort_keys=True,indent=2)+'\n')
# Driver and Rust test call exactly the same original getter list.
names=re.findall(r'int PremiumLimits::(\w+)\(\) const',premium)
(out/'getters.inc').write_text(''.join('OUTPUT('+n+')\n' for n in names))

# Pin all requested source files, even when their owner/UI bodies are not native-tested.
for stem in ['data_user_photos','data_pts_waiter','stickers/data_stickers_set']:
 for ext in ['h','cpp']:read('data/'+stem+'.'+ext)
(out/'source-sha256.json').write_text(json.dumps(hashes,sort_keys=True,indent=2)+'\n')
