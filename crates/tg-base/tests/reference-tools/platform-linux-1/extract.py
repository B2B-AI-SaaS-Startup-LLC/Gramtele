"""Extract a complete dependency-free body, never replace a dependency body."""
import pathlib, sys
root, out = map(pathlib.Path, sys.argv[1:])
s = (root/'lib_base/base/platform/linux/base_info_linux.cpp').read_text()
a = s.index('[[nodiscard]] QString ChassisTypeToString(')
b = s.index('\n[[nodiscard]] bool IsGlibcLess228()', a)
(out/'chassis.hpp').write_text(s[a:b])

s = (root/'lib_base/base/platform/linux/base_linux_dbus_utilities.h').read_text()
a = s.index('enum class StartReply {'); b = s.index('};', a)+2
(out/'start-enum.hpp').write_text('namespace base::Platform::DBus {\n'+s[a:b]+'\n}')
s = (root/'lib_base/base/platform/linux/base_power_save_blocker_linux.cpp').read_text()
a = s.index('uniqueName.erase(0, 1);')
b = s.index('\n\n', a)
(out/'request-name.hpp').write_text(s[a:b])
s = (root/'lib_base/base/platform/linux/base_linux_xdp_utilities.h').read_text()
a = s.index('inline constexpr auto kService')
b = s.index('\n\nstd::string ParentWindowID()', a)
(out/'portal-constants.hpp').write_text('namespace base::Platform::XDP {\n'+s[a:b]+'\n}')
