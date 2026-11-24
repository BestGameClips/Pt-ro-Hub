import {mkdirSync,writeFileSync} from 'node:fs';
const n=32, stride=n*4, maskStride=4, size=40+stride*n+maskStride*n;
const b=Buffer.alloc(22+size);
b.writeUInt16LE(1,2); b.writeUInt16LE(1,4);
b[6]=n;b[7]=n;b.writeUInt16LE(1,10);b.writeUInt16LE(32,12);b.writeUInt32LE(size,14);b.writeUInt32LE(22,18);
b.writeUInt32LE(40,22);b.writeInt32LE(n,26);b.writeInt32LE(n*2,30);b.writeUInt16LE(1,34);b.writeUInt16LE(32,36);
for(let y=0;y<n;y++)for(let x=0;x<n;x++){const i=62+(y*n+x)*4;const bar=x>=6&&x<=25&&((y>=7&&y<=12)||(y>=19&&y<=24)); b[i]=bar?250:220;b[i+1]=bar?250:86;b[i+2]=bar?250:115;b[i+3]=255;}
mkdirSync('src-tauri/icons',{recursive:true});writeFileSync('src-tauri/icons/icon.ico',b);
