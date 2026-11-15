from pathlib import Path
import sys,json,hashlib
root,out=map(Path,sys.argv[1:]);out.mkdir(parents=True,exist_ok=True)
def body(file,signature):
 s=(root/file).read_text();start=s.index('{',s.index(signature));i=start+1;depth=1
 while depth:
  if s[i]=='{':depth+=1
  if s[i]=='}':depth-=1
  i+=1
 return s[start+1:i-1]
f='SourceFiles/storage/download_manager_mtproto.cpp'
s=body(f,'DownloadMtprotoTask::CheckCdnHashResult DownloadMtprotoTask::checkCdnFileHash(')
out.joinpath('hash-body.hpp').write_text('enum class CheckCdnHashResult { NoHash, Invalid, Good };\nstruct CdnFileHash{int limit;QByteArray hash;};\nCheckCdnHashResult Check(const base::flat_map<int64,CdnFileHash> &_cdnFileHashes,int64 offset,bytes::const_span buffer){'+s+'}\n')
s=(root/'SourceFiles/storage/file_upload.cpp').read_text();start=s.index('// max 1mb');end=s.index('[[nodiscard]] const char *ThumbnailFormat')
constants=s[start:end]
a=body('SourceFiles/storage/file_upload.cpp','void Uploader::Entry::setDocSize(')
b=body('SourceFiles/storage/file_upload.cpp','bool Uploader::Entry::setPartSize(')
out.joinpath('upload-layout.hpp').write_text(constants+'\nstruct Layout {int64 docSize=0;int docPartSize=0;ushort docPartsCount=0;bool setPartSize(int partSize){'+b+'}void setDocSize(int64 size){'+a+'}};\n')
files=['SourceFiles/storage/'+stem+ext for stem in ['download_manager_mtproto','file_upload'] for ext in ['.h','.cpp']]
out.joinpath('source-sha256.json').write_text(json.dumps({p:hashlib.sha256((root/p).read_bytes()).hexdigest() for p in files},indent=2)+'\n')
