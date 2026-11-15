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
h=(root/'SourceFiles/core/utils.h').read_text();c=(root/'SourceFiles/core/utils.cpp').read_text()
a=h.index('const static uint32 _md5_block_size');b=h.index('int32 *hashSha1',a)
md5=h[a:b]+c[c.index('namespace {',c.index('// md5 hash')):c.index('\nnamespace {',c.index('char *hashMd5Hex'))]
md5+='\ninline char *hashMd5Hex(const void *data,uint32 len,void *dest){return hashMd5Hex(HashMd5(data,len).result(),dest);}\n'
out.joinpath('md5.hpp').write_text(md5)
f='SourceFiles/storage/localimageloader.cpp'
parts='constexpr auto kPhotoUploadPartSize=32*1024;\nstruct Parts {int64 partssize=0;std::vector<QByteArray> fileparts,thumbparts;QByteArray filemd5,thumbmd5,thumbbytes;'
for name in ['setFileData','setThumbData']:
 arg='filedata' if name=='setFileData' else 'thumbdata'
 parts+=f'void {name}(const QByteArray &{arg})'+'{'+body(f,'void FilePrepareResult::'+name+'(')+'}\n'
out.joinpath('parts.hpp').write_text(parts+'};\n')
rules=''
for signature,wrapper in [('int PhotoSideLimit(bool large)','int PhotoSideLimit(bool large)'),('[[nodiscard]] bool FileThumbnailUploadRequired(','bool FileThumbnailUploadRequired(const QString &filemime,int64 filesize)'),('[[nodiscard]] QString Mp4FileName(','QString Mp4FileName(const QString &name)')]:
 rules+=wrapper+'{'+body(f,signature)+'}\n'
u='SourceFiles/ui/chat/attach/attach_prepare.cpp'
rules+='bool ValidateThumbDimensions(int width,int height){'+body(u,'bool ValidateThumbDimensions(')+'}\n'
rules+='enum class FileType{None,Photo,Video,Music,File};enum class AlbumType{None,PhotoVideo,Music,File};\n'
# Required pure value projection; no pixel/History/Session class bodies are replaced.
# Only the qualified enum alias is rebound; no fake PreparedFile class.
rules+='AlbumType GroupTypeForFile(FileType type,bool groupFiles,bool sendImagesAsPhotos){'+body(u,'[[nodiscard]] AlbumType GroupTypeForFile(').replace('PreparedFile::Type','FileType')+'}\n'
out.joinpath('rules.hpp').write_text(rules)
files=[f'SourceFiles/storage/{s}.{e}' for s in ['localimageloader','storage_media_prepare'] for e in ['h','cpp']]+['SourceFiles/core/utils.h','SourceFiles/core/utils.cpp','SourceFiles/ui/chat/attach/attach_prepare.h',u,'SourceFiles/ui/chat/attach/attach_send_files_way.h','SourceFiles/ui/chat/attach/attach_send_files_way.cpp']
out.joinpath('source-sha256.json').write_text(json.dumps({p:hashlib.sha256((root/p).read_bytes()).hexdigest() for p in files},indent=2)+'\n')
