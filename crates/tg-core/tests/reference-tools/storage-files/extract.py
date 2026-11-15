"""Extract stream expressions, never substitute real model/session method bodies."""
from pathlib import Path
import sys,json,hashlib
root,out=map(Path,sys.argv[1:]);out.mkdir(parents=True,exist_ok=True)
def body(path,signature):
 s=(root/path).read_text();a=s.index(signature);start=s.index('{',a);level=1;i=start+1
 while level:
  if s[i]=='{':level+=1
  if s[i]=='}':level-=1
  i+=1
 return s[start+1:i-1]
peer=body(Path('SourceFiles/storage/serialize_peer.cpp'),'void writePeer(')
doc=body(Path('SourceFiles/storage/serialize_document.cpp'),'void Document::writeToStream(')
def chain(s,start,replace):
 a=s.index(start);b=s.index(';',a)+1;code=s[a:b]
 for source,target in sorted(replace.items(),key=lambda v:-len(v[0])):code=code.replace(source,target)
 return code
user=chain(peer,'stream\n\t\t\t<< user->firstName',{'user->firstName':'first','user->lastName':'last','user->phone()':'phone','user->username()':'username','user->accessHash()':'access','user->flags()':'flags','botInlinePlaceholder':'placeholder','user->lastseen().serialize()':'lastseen','user->isContact() ? 1 : 0':'contact','user->isBot() ? user->botInfo->version : -1':'botVersion','user->isBot()\n\t\t\t\t&& user->botInfo\n\t\t\t\t&& user->botInfo->supportsGuestChat':'guest'})
chat=chain(peer,'stream\n\t\t\t<< chat->name()',{'chat->name()':'name','chat->count':'count','chat->date':'date','chat->version()':'version','chat->flags()':'flags','chat->inviteLink()':'invite'})
channel=chain(peer,'stream\n\t\t\t<< channel->name()',{'channel->name()':'name','channel->accessHash()':'access','channel->date':'date','channel->flags()':'flags','channel->inviteLink()':'invite'})
dochead=chain(doc,'stream\n\t\t<< quint64(document->id)',{'document->id':'id','document->_access':'access','document->date':'date','document->_fileReference':'reference','kVersionTag':'qint32(0x7fffffff)','kVersion':'qint32(6)','document->filename()':'name','document->mimeString()':'mime','document->_dc':'dc','document->size':'size','document->dimensions.width()':'width','document->dimensions.height()':'height','document->type':'type'})
out.joinpath('stream-chains.hpp').write_text('''// Exact original QDataStream expressions over explicit scalar projections.
void WriteUser(QDataStream &stream, QString first,QString last,QString phone,QString username,quint64 access,qint32 flags,QString placeholder,quint32 lastseen,qint32 contact,qint32 botVersion,bool guest){'''+user+'''}
void WriteChat(QDataStream &stream,QString name,qint32 count,qint32 date,qint32 version,qint32 field1,qint32 field2,quint32 flags,QString invite){'''+chat+'''}
void WriteChannel(QDataStream &stream,QString name,quint64 access,qint32 date,quint32 flags,QString invite){'''+channel+'''}
void WriteDocumentHead(QDataStream &stream,quint64 id,quint64 access,qint32 date,QByteArray reference,QString name,QString mime,qint32 dc,quint32 size,qint32 width,qint32 height,qint32 type){'''+dochead+'}\n')
archive=Path('SourceFiles/storage/storage_folder_archive.cpp')
full=(root/archive).read_text()
functions=[]
for signature in ['[[nodiscard]] bool AlreadyCompressedName(', '[[nodiscard]] QString DedupedName(', 'void FillEntryDate(', 'int64 ArchiveSizeEstimate(']:
 a=full.index(signature);b=full.index('{',a);functions.append(full[a:b+1]+body(archive,signature)+'}')
out.joinpath('archive-pure.hpp').write_text('\n'.join(functions).replace('ArchiveSizeEstimate(', 'NativeArchiveSizeEstimate('))
files=['SourceFiles/storage/'+stem+ext for stem in ['serialize_peer','serialize_document','storage_folder_archive','file_download','file_download_mtproto','file_download_web','streamed_file_downloader'] for ext in ['.h','.cpp']]+['SourceFiles/ui/image/image_location.cpp','SourceFiles/ui/image/image_location.h','SourceFiles/storage/download_manager_mtproto.h']
out.joinpath('source-sha256.json').write_text(json.dumps({str(p):hashlib.sha256((root/p).read_bytes()).hexdigest() for p in map(Path,files)},indent=2)+'\n')
# Entire original archive writer and its real QFile callbacks, no UI class bodies.
writer=[]
for signature in ['[[nodiscard]] bool StepProgress(', '[[nodiscard]] QString ArchiveDirectory(', '[[nodiscard]] QString ArchiveFileTemplate(', '[[nodiscard]] ArchiveWriteResult WriteArchiveFile(']:
 a=full.index(signature);b=full.index('{',a);writer.append(full[a:b+1]+body(archive,signature)+'}')
cls=full.index('class ZipToFile {');end=full.index('\n};',cls)+3
state=full[full.index('struct WriteState {'):full.index('\n};',full.index('struct WriteState {'))+3]
out.joinpath('archive-writer.hpp').write_text('''using Status = ArchiveWriteResult::Status;
constexpr auto kArchiveChunkSize = int64(1024 * 1024);
constexpr auto kArchiveMemLevel = 8;
constexpr auto kUtf8NamesFlag = uLong(1) << 11;
constexpr auto kDirectoryAttribute = uLong(0x10);
'''+state+'\n'+full[cls:end]+'\n'+'\n'.join(writer))
location=body(Path('SourceFiles/ui/image/image_location.cpp'),'QByteArray StorageFileLocation::serialize() const')
wire=chain(location,'stream\n\t\t\t<< quint16(_dcId)',{'_dcId':'dc','_sizeLetter':'letter','_id':'id','_accessHash':'access','_volumeId':'volume','_inMessageId':'message','_fileReference':'reference'})
reader=body(Path('SourceFiles/storage/serialize_document.cpp'),'DocumentData *Document::readFromStreamHelper(')
header=reader[reader.index('quint64 id, access;'):reader.index('QVector<MTPDocumentAttribute>')]
header=header.replace('kVersionTag','qint32(INT32_MAX)')
out.joinpath('extra-wire.hpp').write_text('''void WriteStorage(QDataStream &stream,quint16 dc,quint8 typeWithFlags,quint8 letter,qint32 field1,quint64 id,quint64 access,quint64 volume,qint32 field2,qint32 message,QByteArray reference){'''+wire+'''}
QJsonObject ReadDocumentHeader(const QByteArray &bytes,int streamAppVersion){QDataStream stream(bytes);stream.setVersion(QDataStream::Qt_5_1);'''+header+'''
return {{"read_id",double(id)},{"read_dc",dc},{"read_version",version},{"read_position",double(stream.device()->pos())},{"read_ok",stream.status()==QDataStream::Ok}};
}
''')
# Whole current writer body, projected only at accessor/argument boundaries.
# This executes original branches as well as all tail stream expressions.
projected=doc
replacements={
 'if (const auto sticker = document->sticker())':'if (hasSticker)',
 'sticker->setType':'setType','sticker->set.id':'setId','sticker->alt':'alt',
 'Data::StickersType::Emoji':'2','Data::StickersType::Masks':'1',
 'document->isPremiumSticker()':'premiumSticker','document->isPremiumEmoji()':'premiumEmoji',
 'document->emojiUsesTextColor()':'textColor','document->hasDuration()':'hasDuration',
 'document->duration()':'duration','document->thumbnailLocation()':'thumb',
 'document->videoThumbnailLocation()':'videoThumb','document->thumbnailByteSize()':'thumbSize',
 'document->videoThumbnailByteSize()':'videoThumbSize','document->inlineThumbnailIsPath()':'inlinePath',
 'document->inlineThumbnailBytes()':'inlineBytes','document->dimensions.width()':'width',
 'document->dimensions.height()':'height','document->filename()':'name','document->mimeString()':'mime',
 'document->_fileReference':'reference','document->_access':'access','document->_dc':'dc',
 'document->date':'date','document->size':'size','document->id':'id','document->type':'type',
 'writeImageLocation(':'WriteImageBlob(', 'StickerDocument':'3',
 'kVersionTag':'qint32(INT32_MAX)','kVersion':'qint32(6)',
}
for before,after in sorted(replacements.items(),key=lambda p:-len(p[0])):projected=projected.replace(before,after)
out.joinpath('document-current.hpp').write_text('''enum StickerSetType {StickerSetTypeEmpty=0,StickerSetTypeID=1,StickerSetTypeShortName=2,StickerSetTypeEmoji=3,StickerSetTypeMasks=4};
void WriteImageBlob(QDataStream &stream,const QByteArray &bytes){stream << qint32(INT32_MIN) << bytes;}
void WriteDocumentCurrent(QDataStream &stream,quint64 id,quint64 access,qint32 date,QByteArray reference,QString name,QString mime,qint32 dc,quint32 size,qint32 width,qint32 height,qint32 type,bool hasSticker,QString alt,int setType,quint64 setId,bool hasDuration,qint64 duration,bool premiumSticker,bool premiumEmoji,bool textColor,QByteArray thumb,qint32 thumbSize,QByteArray videoThumb,qint32 videoThumbSize,bool inlinePath,QByteArray inlineBytes){'''+projected+'}\n')
