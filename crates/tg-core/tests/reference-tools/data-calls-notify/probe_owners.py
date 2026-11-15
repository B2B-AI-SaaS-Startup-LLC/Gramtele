"""Try actual full owner-dependent translation units without stubs or edits.
Usage inside nix develop: python probe_owners.py NINJA BUILD_DIR TELEGRAM_ROOT LOG
Nonzero compile exits are recorded, not relabelled as native proof.
"""
import subprocess,shlex,sys
from pathlib import Path
ninja,build,root,log=sys.argv[1:5];extra_includes=sys.argv[5:];commands=subprocess.check_output([ninja,'-C',build,'-t','commands'],text=True).splitlines()
command=next(shlex.split(s) for s in commands if ' -c ' in s and '/main.cpp' in s)
base=[];i=0
while i<len(command):
 if command[i] in ['-o','-c','-MT','-MF']:i+=2
 elif command[i] in ['-MD','-MMD']:i+=1
 else:base.append(command[i]);i+=1
# Use the real application PCH and real Qt include directories, not replacement
# declarations for missing model/UI members.
qt=Path(next(s for s in base if s.endswith('/include/QtCore'))).parent
base+=[f'-I{path}' for path in extra_includes]
base+=['-DQT_NO_KEYWORDS','-DCRL_USE_QT']+[f'-I{qt/name}' for name in ['QtGui','QtWidgets','QtOpenGL','QtOpenGLWidgets']]+['-include',str(Path(root)/'SourceFiles/stdafx.h')]
names=['data/data_group_call.cpp','data/notify/data_notify_settings.cpp','data/data_media_preload.cpp','data/data_statistics_chart.cpp']
with open(log,'w') as out:
 for name in names:
  out.write('\n=== '+name+' (actual full source, syntax-only) ===\n');out.flush()
  result=subprocess.run(base+['-fsyntax-only',str(Path(root)/'SourceFiles'/name)],cwd=build,stdout=out,stderr=subprocess.STDOUT)
  out.write('exit='+str(result.returncode)+'\n');out.flush()
