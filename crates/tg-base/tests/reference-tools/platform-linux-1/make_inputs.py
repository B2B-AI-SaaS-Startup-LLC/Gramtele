import json, sys
rows=[]
def add(op,*args): rows.append(dict(op=op,args=args))
for s in ['', 'a','Desktop','0123456789012345','01234567890123456','😀'*7,'😀'*8,'a_ b\n c\t\x01','HP EliteBook 840 G8 Notebook PC','HP '+'x'*40+' PC','HP mobile workstation PC','  ab  cd  ','a\u00a0b','a\u2028b']:
 for op in ['model_ok','simplify','product','good','finalize']:add(op,s)
for n in range(0,36):add('chassis',n)
for v in ['', '2.27','2.28','2.28.0','2.28.1','2','2x','0','0.0','1.99','3.0','2.028','02.28','2147483648.1','.2',' 2.4','2.2147483648','2..1','-1','2.28x','+2.4','\t2.4','2. 28',' 2.4','2.+29']:
 add('version',v)
 for name in ['glibc','musl','']:add('old_glibc',name,v)
for s in ['', '  --foo x ', "one 'two three'", 'a\tb\nc', '"a\\$b"', '"a\\`b"', "a'b c'd", "a\\ b", "'", '"bad', 'a\\', "$'\\n\\t\\x10\\x11\\101'", "$'\\x1'", "$'\\q\\cA\\777'", "'' \"\"", '$HOME ~x #x', 'a;b & (x)', '\u200b\u00a0', "a'\\b\"c\x01\x7f\u0085", '\x00\n', '😀']:
 for op in ['split','quote']:add(op,s)
for a in [[],[''],['/opt/a b/app','--flag','--','%u'],["it's",'a\nb','\t','x']]:add('join',*a)
for s in ['', 'GNOME: GNOME : :KDE', ':\t:\n:','A\u00a0B:C']:add('desktop',s)
for op in ['start_enum','load_missing','symbol_missing']:add(op)
for name in [':1.2',':123.987',':1.2.3',':0.0']:add('dbus_request_path',name,'desktop_app4294967295')
sys.stdout.write(''.join(json.dumps(r,ensure_ascii=False)+'\n' for r in rows))
