#include "base/platform/base_platform_info.h"
#include "base/platform/linux/base_linux_library.h"
#include "start-enum.hpp"
#include <QtCore/QCoreApplication>
#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QVersionNumber>
#include <KShell>
#include <iostream>
#include <dlfcn.h>
#include "chassis.hpp"
#include "portal-constants.hpp"
int main(int argc, char **argv) {
 QCoreApplication app(argc,argv);
 std::string line;
 while(std::getline(std::cin,line)) {
  auto obj=QJsonDocument::fromJson(QByteArray::fromStdString(line)).object();
  auto op=obj["op"].toString(); auto a=obj["args"].toArray();
  auto s=[&](int i){return a[i].toString();}; QJsonValue value;
  using namespace base::Platform;
  if(op=="model_ok") value=IsDeviceModelOk(s(0));
  else if(op=="simplify") value=SimplifyDeviceModel(s(0));
  else if(op=="product") value=ProductNameToDeviceModel(s(0));
  else if(op=="good") value=SimplifyGoodDeviceModel(s(0),{"notebook","desktop","mobile","workstation","pc"});
  else if(op=="finalize") value=FinalizeDeviceModel(s(0));
  else if(op=="chassis") value=ChassisTypeToString(a[0].toInt());
  else if(op=="version") value=!QVersionNumber::fromString(s(0)).isNull();
  else if(op=="old_glibc") value=(s(0)=="glibc" && !s(1).isEmpty() && QVersionNumber::fromString(s(1))<QVersionNumber(2,28));
  else if(op=="split") value=QJsonArray::fromStringList(KShell::splitArgs(s(0)));
  else if(op=="quote") value=KShell::quoteArg(s(0));
  else if(op=="join") {QStringList list; for(auto v:a) list.push_back(v.toString()); value=KShell::joinArgs(list);}
  else if(op=="desktop") {QStringList list; for(auto item:s(0).split(':')) {auto v=item.simplified(); if(!v.isEmpty())list.push_back(v);}value=QJsonArray::fromStringList(list);}
  else if(op=="dbus_request_path") {
   auto uniqueName=s(0).toStdString();
#include "request-name.hpp"
   value=QString::fromStdString(XDP::kObjectPath + std::string("/request/") + uniqueName + '/' + s(1).toStdString());
  }
  else if(op=="start_enum") {value=QJsonArray{int(DBus::StartReply::Success),int(DBus::StartReply::AlreadyRunning)};}
  else if(op=="load_missing") value=bool(LoadLibrary("/tg-native-no-such-library.so"));
  else if(op=="symbol_missing") {auto lib=LoadLibrary("libc.so.6");value=(LoadSymbolGeneric(lib,"tg_symbol_does_not_exist")!=nullptr);}
  else {std::cerr<<"unknown op";return 2;}
  obj["expected"]=value;
  std::cout<<QJsonDocument(obj).toJson(QJsonDocument::Compact).constData()<<'\n';
 }
}
