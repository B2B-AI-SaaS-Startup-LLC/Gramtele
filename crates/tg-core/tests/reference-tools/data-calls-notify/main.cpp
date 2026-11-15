#include "data/data_statistics_chart.h"
#include "statistics/statistics_data_deserialize.h"
#include "data/raw/raw_countries_bounds.h"
#include "parts.cpp"
void doubles(const std::vector<double> &v){for(auto n:v)std::cout<<n<<";";}
void hex(const QString &s){std::cout<<s.toUtf8().toHex().toStdString();}
int main(int argc,char**argv){QCoreApplication app(argc,argv);std::cout<<std::setprecision(17);
for(const auto &[code,b]:Raw::CountryBounds()){std::cout<<"country,"<<code.toStdString()<<","<<b.minLat<<","<<b.minLon<<","<<b.maxLat<<","<<b.maxLon<<"\n";}
for(int n: {0,1,2,5,30,31}){Data::StatisticalChart c;for(int i=0;i<n;++i)c.x.push_back(i*1000);c.timeStep=1000;c.measure();for(double v:{-0.5,0.,0.1,0.5,0.9,1.,1.5}){auto a=c.findStartIndex(v);std::cout<<"index,"<<n<<","<<v<<","<<a<<","<<c.findEndIndex(0,v)<<","<<(n?c.findIndex(0,n-1,v):-1)<<"\n";}}
for(int n:{0,1,29,30,31,60}){std::vector<int64> a;for(int i=0;i<n;++i)a.push_back(-i-1);Statistic::SegmentTree t(a);for(int from:{-1,0,1,10,60})for(int to:{-1,0,10,29,31,61})if(from<=to)std::cout<<"tree,"<<n<<","<<from<<","<<to<<","<<t.rMinQ(from,to)<<","<<t.rMaxQ(from,to)<<"\n";}
for(int n=0;n<4;++n){base::flat_map<uint32,QByteArray> parts;for(int i=0;i<n;++i)parts.emplace(i*131072,QByteArray::fromHex(QByteArray::number(i,16).rightJustified(2,'0')+"00ff"));std::cout<<"parts,"<<n<<","<<Media::Streaming::SerializeComplexPartsMap(parts).toHex().toStdString()<<"\n";}
std::string input;int i=0;while(std::getline(std::cin,input)){auto c=Statistic::StatisticalChartFromJSON(QByteArray::fromStdString(input));std::cout<<"json,"<<i++<<","<<c.lines.size()<<","<<c.maxValue<<","<<c.minValue<<","<<c.timeStep<<","<<c.oneDayPercentage<<","<<c.isFooterHidden<<","<<c.hasPercentages<<","<<c.weekFormat<<","<<int(c.currency)<<","<<c.defaultZoomXIndex.min<<","<<c.defaultZoomXIndex.max<<",";doubles(c.x);std::cout<<",";doubles(c.xPercentage);std::cout<<"\n";for(auto &l:c.lines){std::cout<<"line,"<<l.id<<",";hex(l.idString);std::cout<<",";hex(l.name);std::cout<<",";hex(l.colorKey);std::cout<<","<<l.maxValue<<","<<l.minValue<<","<<l.isHiddenOnStart<<",";for(auto y:l.y)std::cout<<y<<";";std::cout<<"\n";}}
}
