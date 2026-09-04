// 生成一个 1024x1024 的源图标（纯 Node，无第三方依赖）
// 蓝色圆角底 + 白色对勾，供 `tauri icon` 生成各平台图标
const zlib = require("zlib");
const fs = require("fs");

const W = 1024;
const H = 1024;
const BG = [22, 119, 255, 255]; // #1677ff
const FG = [255, 255, 255, 255];

// 圆角矩形判定
function inRoundRect(x, y, w, h, r) {
  if (x < r && y < r) return Math.hypot(x - r, y - r) <= r;
  if (x > w - r && y < r) return Math.hypot(x - (w - r - 1), y - r) <= r;
  if (x < r && y > h - r) return Math.hypot(x - r, y - (h - r - 1)) <= r;
  if (x > w - r && y > h - r)
    return Math.hypot(x - (w - r - 1), y - (h - r - 1)) <= r;
  return true;
}

// 对勾路径（两段线段），宽度 w 的粗线
function onCheck(x, y) {
  // 对勾三个关键点（1024 坐标系）
  const p1 = [300, 540];
  const p2 = [450, 690];
  const p3 = [740, 360];
  const stroke = 72;
  function distToSeg(px, py, ax, ay, bx, by) {
    const dx = bx - ax;
    const dy = by - ay;
    const t = Math.max(
      0,
      Math.min(1, ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)),
    );
    const qx = ax + t * dx;
    const qy = ay + t * dy;
    return Math.hypot(px - qx, py - qy);
  }
  const d1 = distToSeg(x, y, p1[0], p1[1], p2[0], p2[1]);
  const d2 = distToSeg(x, y, p2[0], p2[1], p3[0], p3[1]);
  return d1 <= stroke / 2 || d2 <= stroke / 2;
}

const raw = Buffer.alloc(H * (1 + W * 4));
let o = 0;
for (let y = 0; y < H; y++) {
  raw[o++] = 0; // filter: none
  for (let x = 0; x < W; x++) {
    let px;
    if (!inRoundRect(x, y, W, H, 180)) {
      px = [0, 0, 0, 0]; // 透明
    } else if (onCheck(x, y)) {
      px = FG;
    } else {
      px = BG;
    }
    raw[o++] = px[0];
    raw[o++] = px[1];
    raw[o++] = px[2];
    raw[o++] = px[3];
  }
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const typeBuf = Buffer.from(type, "ascii");
  const body = Buffer.concat([typeBuf, data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}

function crc32(buf) {
  let c = ~0;
  for (let i = 0; i < buf.length; i++) {
    c ^= buf[i];
    for (let k = 0; k < 8; k++) c = c & 1 ? (c >>> 1) ^ 0xedb88320 : c >>> 1;
  }
  return ~c >>> 0;
}

const sig = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(W, 0);
ihdr.writeUInt32BE(H, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 6; // RGBA
const png = Buffer.concat([
  sig,
  chunk("IHDR", ihdr),
  chunk("IDAT", zlib.deflateSync(raw, { level: 9 })),
  chunk("IEND", Buffer.alloc(0)),
]);
fs.writeFileSync("app-icon.png", png);
console.log("wrote app-icon.png", png.length, "bytes");
