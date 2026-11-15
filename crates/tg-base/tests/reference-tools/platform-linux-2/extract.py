from pathlib import Path
import sys
root,out=map(Path,sys.argv[1:])
s=(root/'lib_base/base/platform/linux/base_linux_xsettings.cpp').read_text()
a=s.index('class XSettings::PropertyValue {');b=s.index('\nclass XSettings::Private',a)
property_value=s[a:b].replace('class XSettings::PropertyValue','class PropertyValue')
a=s.index('\tstatic int round_to_nearest_multiple_of_4');b=s.index('\n\tconst Connection connection;',a)
parse=s[a:b]
(out/'parser.hpp').write_text(property_value+'\nstruct Parser {\n enum class Type {Integer,String,Color};\n'+parse+'\n xcb_connection_t *connection=nullptr; base::flat_map<QByteArray,PropertyValue> settings;\n};\n')
s=(root/'lib_base/base/platform/linux/base_global_shortcuts_linux.cpp').read_text()
a=s.index('static const auto KeyToString =');b=s.index('\n\t// Mouse.',a)
(out/'key-table.hpp').write_text(s[a:b])

s=(root/'lib_base/base/platform/linux/base_system_media_controls_linux.cpp').read_text()
a=s.index('std::string ConvertPlaybackStatus(');b=s.index('\n} // namespace',a)
(out/'media-helpers.hpp').write_text(s[a:b])
s=(root/'lib_base/base/platform/linux/base_global_shortcuts_linux.cpp').read_text()
a=s.index('extern "C" {');b=s.index('} // extern "C"',a)+len('} // extern "C"')
(out/'record-abi.hpp').write_text(s[a:b])
