import assert from 'node:assert/strict'
import { deflateSync } from 'node:zlib'
import { spawnSync } from 'node:child_process'

// Valid, deterministic PNGs keep the same fixtures usable in external renderers.
export function image(width, height) {
  function chunk(type, data) {
    const content = Buffer.concat([Buffer.from(type), data])
    let crc = 0xffffffff
    for (const byte of content) {
      crc ^= byte
      for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0)
    }
    const result = Buffer.alloc(data.length + 12)
    result.writeUInt32BE(data.length); content.copy(result, 4); result.writeUInt32BE((crc ^ 0xffffffff) >>> 0, result.length - 4)
    return result
  }
  const header = Buffer.alloc(13)
  header.writeUInt32BE(width); header.writeUInt32BE(height, 4); header[8] = 8; header[9] = 2
  const pixels = Buffer.alloc((width * 3 + 1) * height)
  for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
    const offset = y * (width * 3 + 1) + 1 + x * 3
    const edge = x < 8 || y < 8 || x >= width - 8 || y >= height - 8
    pixels[offset] = edge ? 20 : 60 + Math.floor(x / width * 100)
    pixels[offset + 1] = edge ? 40 : 100 + Math.floor(y / height * 100)
    pixels[offset + 2] = edge ? 60 : 210
  }
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', header), chunk('IDAT', deflateSync(pixels)), chunk('IEND', Buffer.alloc(0))])
}
export function documentXml(bytes) {
  const result = spawnSync('python3', ['-c', 'import io,sys,zipfile; sys.stdout.buffer.write(zipfile.ZipFile(io.BytesIO(sys.stdin.buffer.read())).read("word/document.xml"))'], { input: bytes })
  assert.equal(result.status, 0, result.stderr.toString())
  return result.stdout.toString()
}

export function packageParts(bytes) {
  const result = spawnSync('python3', ['-c', `
import io,sys,zipfile,json,base64
with zipfile.ZipFile(io.BytesIO(sys.stdin.buffer.read())) as source:
 print(json.dumps({name:base64.b64encode(source.read(name)).decode() for name in source.namelist()}))
`], { input: bytes, maxBuffer: 16 * 1024 * 1024 })
  assert.equal(result.status, 0, result.stderr.toString())
  return Object.fromEntries(Object.entries(JSON.parse(result.stdout)).map(([name, value]) => [name, Buffer.from(value, 'base64')]))
}

export function editDocument(bytes, edit) {
  const parts = packageParts(bytes)
  parts['word/document.xml'] = Buffer.from(edit(parts['word/document.xml'].toString()))
  const result = spawnSync('python3', ['-c', `
import io,sys,zipfile,json,base64
parts=json.load(sys.stdin)
output=io.BytesIO()
with zipfile.ZipFile(output,'w',zipfile.ZIP_DEFLATED) as target:
 for name,data in parts.items(): target.writestr(zipfile.ZipInfo(name, date_time=(1980,1,1,0,0,0)),base64.b64decode(data),compress_type=zipfile.ZIP_DEFLATED)
sys.stdout.buffer.write(output.getvalue())
`], { input: JSON.stringify(Object.fromEntries(Object.entries(parts).map(([name, value]) => [name, value.toString('base64')]))), maxBuffer: 16 * 1024 * 1024 })
  assert.equal(result.status, 0, result.stderr.toString())
  return result.stdout
}
