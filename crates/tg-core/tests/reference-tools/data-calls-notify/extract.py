from pathlib import Path
import sys,re,json,hashlib
root,out=map(Path,sys.argv[1:3]);out.mkdir(parents=True,exist_ok=True);hashes={}
def read(n):
 p=root/'SourceFiles'/n;hashes[n]=hashlib.sha256(p.read_bytes()).hexdigest();return p.read_text()
def fun(s,n):
 a=s.index(n);return s[a:s.index('\n}',a)+2]+'\n'
s=read('data/data_statistics_chart.cpp');a=s.index('void StatisticalChart::measure()');b=s.index('\n\tdaysLookup.clear()',a)
# Exact numeric phase, omit only actual UI labels/font-width phase. No fake font.
pure='#include "data/data_statistics_chart.h"\nnamespace Data {\n'+s[a:b]+'\n\tdaysLookup.clear();\n\toneDayPercentage = timeStep / float64(end - start);\n}\n'
for n in ['QString StatisticalChart::getDayString(', 'int StatisticalChart::findStartIndex(', 'int StatisticalChart::findEndIndex(', 'int StatisticalChart::findIndex(']:pure+=fun(s,n)
pure+='}\n';(out/'chart.cpp').write_text(pure)
s=read('statistics/statistics_data_deserialize.cpp');credit=read('ui/text/format_values.h');line=next(l for l in credit.splitlines() if 'inline const QString kCreditsCurrency =' in l)
(out/'parse.cpp').write_text('#include "data/data_statistics_chart.h"\n#include "base/debug_log.h"\nnamespace Ui {\n'+line+'\n}\n'+s[s.index('namespace Statistic {'):])
for stem in ['data/data_group_call','data/notify/data_notify_settings','data/data_media_preload','data/data_statistics_chart','data/raw/raw_countries_bounds','statistics/statistics_data_deserialize','statistics/segment_tree']:
 for ext in ['h','cpp']:read(stem+'.'+ext)
(out/'source-sha256.json').write_text(json.dumps(hashes,indent=2,sort_keys=True)+'\n')

s=read('media/streaming/media_streaming_reader.cpp');loader=read('media/streaming/media_streaming_loader.h');line=next(l for l in loader.splitlines() if 'constexpr auto kPartSize =' in l).strip().replace('static ','')
(out/'parts.cpp').write_text('namespace Media::Streaming {\n'+line+'\n'+fun(s,'QByteArray SerializeComplexPartsMap(')+'}\n')
(out/'source-sha256.json').write_text(json.dumps(hashes,indent=2,sort_keys=True)+'\n')
