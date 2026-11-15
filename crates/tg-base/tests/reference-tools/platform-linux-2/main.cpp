#include <QtCore/QCoreApplication>
#include <QtCore/QByteArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QJsonArray>
#include <QtCore/QVariant>
#include <QtCore/QtEndian>
#include <QtGui/QColor>
#include <xkbcommon/xkbcommon-keysyms.h>
#include "base/platform/linux/base_linux_xcb_library.h"
#include "base/platform/base_platform_system_media_controls.h"
using namespace base;
using namespace base::Platform;
#include "media-helpers.hpp"
#include "record-abi.hpp"
using PropertyChangeFunc=Fn<void(xcb_connection_t*,const QByteArray&,const QVariant&)>;
#include "parser.hpp"
#include <iostream>
int main(int argc,char**argv){
 QCoreApplication app(argc,argv);std::string line;
 while(std::getline(std::cin,line)){
  auto obj=QJsonDocument::fromJson(QByteArray::fromStdString(line)).object();auto op=obj["op"].toString();QJsonValue result;
  if(op=="parse"){
   Parser parser;auto events=QJsonArray{};
   for(auto b:obj["blobs"].toArray())parser.populateSettings(QByteArray::fromHex(b.toString().toLatin1()));
   auto records=QJsonArray{};for(auto &[key,p]:parser.settings){QJsonObject r{{"name",QString::fromLatin1(key.toHex())},{"serial",p.last_change_serial}};
    if(!p.value.isValid())r["type"]="invalid";
    else if(p.value.metaType()==QMetaType::fromType<int>()){r["type"]="int";r["value"]=p.value.toInt();}
    else if(p.value.metaType()==QMetaType::fromType<QByteArray>()){r["type"]="string";r["value"]=QString::fromLatin1(p.value.toByteArray().toHex());}
    else {auto c=p.value.value<QColor>();r["type"]="color";r["valid"]=c.isValid();r["value"]=QJsonArray{c.red(),c.green(),c.blue(),c.alpha()};}records.push_back(r);}
   result=records;
  }else if(op=="keys"){
#include "key-table.hpp"
   QJsonArray rows;for(const auto &[key,value]:KeyToString)rows.push_back(QJsonArray{qint64(key),value});result=rows;
  }else if(op=="scale") { auto value=int(obj["value"].toInteger()); result=qint64(value*1000);
  }else if(op=="playback"){result=QString::fromStdString(ConvertPlaybackStatus(static_cast<SystemMediaControls::PlaybackStatus>(obj["value"].toInt())));
  }else if(op=="loop"){result=QString::fromStdString(ConvertLoopStatus(static_cast<SystemMediaControls::LoopStatus>(obj["value"].toInt())));
  }else if(op=="command"){result=int(EventToCommand(obj["value"].toString().toStdString()));
  }else if(op=="loop_command"){result=int(LoopStatusToCommand(obj["value"].toString().toStdString()));
  }else if(op=="abi") {result=QJsonArray{int(sizeof(xcb_screen_t)),int(sizeof(xcb_setup_t)),int(sizeof(xcb_generic_event_t)),int(sizeof(xcb_get_window_attributes_reply_t)),int(sizeof(xcb_record_range_t)),int(sizeof(xcb_record_enable_context_reply_t))};
  }else if(op=="mouse_descriptor") {uint64_t shift=std::numeric_limits<uint64_t>::max()-100;result=QString::number(shift+uint8_t(obj["value"].toInt()));
  }else if(op=="mask"){
   auto old=quint32(obj["old"].toInteger());auto mask=quint32(obj["mask"].toInteger());auto mode=obj["mode"].toInt();
   if((mode==0&&(old&mask))||(mode==1&&!(old&mask))||old==mask)result=QJsonValue::Null;
   else result=qint64(mode==0?old|mask:mode==1?old&~mask:mask);
  }else return 2;
  obj["expected"]=result;std::cout<<QJsonDocument(obj).toJson(QJsonDocument::Compact).constData()<<'\n';
 }
}
