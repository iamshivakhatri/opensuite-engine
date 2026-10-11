/** Build declarations and JSON inputs from the Rust compiler's N-API type records. */
import { readFileSync, writeFileSync, readdirSync } from 'node:fs'
import { join } from 'node:path'
import ts from 'typescript'
import { generateTypeDef } from '@napi-rs/cli'

export function inputContracts(source, integerFields = new Map()) {
  const file = ts.createSourceFile('native.d.ts', source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS)
  if (file.parseDiagnostics.length) throw new Error('Generated native declarations contain invalid TypeScript')
  const interfaces = new Map(file.statements.filter(ts.isInterfaceDeclaration).map((node) => [node.name.text, node]))
  const docs = (node) => {
    const blocks = ts.getJSDocCommentsAndTags(node).filter(ts.isJSDoc)
    let description = '', patch = {}
    for (const block of blocks) {
      if (typeof block.comment === 'string') description += block.comment + '\n'
      for (const tag of block.tags ?? []) {
        if (tag.tagName.text === 'schema') patch = { ...patch, ...JSON.parse(tag.comment) }
      }
    }
    return { description: description.trim(), patch }
  }
  const merge = (base, patch) => {
    const result = { ...base }
    for (const [key, value] of Object.entries(patch)) {
      result[key] = value && typeof value === 'object' && !Array.isArray(value)
        ? merge(base[key] ?? {}, value) : value
    }
    return result
  }
  const schema = (node, stack = []) => {
    if (!node) throw new Error('Missing native input type')
    if (node.kind === ts.SyntaxKind.StringKeyword) return { type: 'string' }
    if (node.kind === ts.SyntaxKind.NumberKeyword) return { type: 'number' }
    if (node.kind === ts.SyntaxKind.BooleanKeyword) return { type: 'boolean' }
    if (ts.isLiteralTypeNode(node)) {
      if (node.literal.kind === ts.SyntaxKind.NullKeyword) return { type: 'null' }
      if (ts.isStringLiteral(node.literal)) return { type: 'string', const: node.literal.text }
      throw new Error(`Unsupported literal input: ${node.getText(file)}`)
    }
    if (ts.isUnionTypeNode(node)) {
      const members = node.types.filter((t) => t.kind !== ts.SyntaxKind.UndefinedKeyword && !(ts.isLiteralTypeNode(t) && t.literal.kind === ts.SyntaxKind.NullKeyword))
      const choices = members.map((member) => schema(member, stack))
      if (choices.length === 1) return choices[0]
      if (choices.every((choice) => choice.type === 'string' && 'const' in choice)) return { type: 'string', enum: choices.map((choice) => choice.const) }
      return { anyOf: choices }
    }
    if (ts.isArrayTypeNode(node)) return { type: 'array', items: schema(node.elementType, stack) }
    if (ts.isTypeReferenceNode(node)) {
      const name = node.typeName.getText(file)
      if (name === 'Array') return { type: 'array', items: schema(node.typeArguments[0], stack) }
      if (name === 'Buffer') return { type: 'string', contentEncoding: 'base64', 'x-native-binary': true }
      const definition = interfaces.get(name)
      if (!definition || stack.includes(name)) throw new Error(`Unknown or recursive native input type: ${name}`)
      const properties = {}, required = []
      for (const member of definition.members) {
        if (!ts.isPropertySignature(member)) throw new Error(`Unsupported native input member: ${name}`)
        const key = member.name.getText(file)
        const doc = docs(member)
        const field = merge(schema(member.type, [...stack, name]), integerFields.get(`${name}.${key}`) ?? {})
        properties[key] = merge(field, doc.patch)
        if (doc.description) properties[key].description = doc.description
        if (!member.questionToken) required.push(key)
      }
      return merge({ type: 'object', properties, required, additionalProperties: false }, docs(definition).patch)
    }
    throw new Error(`Unsupported native input: ${node.getText(file)}`)
  }
  const hasBinary = (value) => value && typeof value === 'object' && (value['x-native-binary'] === true || Object.values(value).some(hasBinary))
  const contracts = {}
  for (const node of file.statements.filter(ts.isFunctionDeclaration)) {
    const name = node.name.text
    if (name === 'getDocxCapabilities') continue
    const args = [...node.parameters]
    if (args[0]?.type?.getText(file) === 'Buffer') args.shift()
    if (args.length > 1) throw new Error(`${name} must accept one JSON input object after document bytes`)
    const doc = docs(node)
    const inputSchema = merge(args.length ? schema(args[0].type) : { type: 'object', properties: {}, required: [], additionalProperties: false }, doc.patch)
    if (inputSchema.type !== 'object') throw new Error(`${name} input must be an object`)
    for (const key of inputSchema.required ?? []) if (!inputSchema.properties[key]) throw new Error(`${name}: required field ${key} does not exist`)
    contracts[name] = { description: doc.description || name.replace(/([a-z])([A-Z])/g, '$1 $2'), inputSchema, jsonInput: !hasBinary(inputSchema) }
  }
  return contracts
}

export function rustIntegerFields(root) {
  const fields = new Map()
  const camel = (value) => value.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase())
  for (const name of readdirSync(join(root, 'src')).filter((name) => name.endsWith('.rs'))) {
    const source = readFileSync(join(root, 'src', name), 'utf8')
    // N-API structs have ordinary named fields. TypeScript supplies the full object shape.
    for (const match of source.matchAll(/pub struct (\w+)\s*\{([\s\S]*?)^\}/gm)) {
      for (const field of match[2].matchAll(/^\s*pub (\w+):\s*([^\n]+),/gm)) {
        const type = field[2].replace(/^Option<(.+)>$/, '$1')
        const scalar = type.replace(/^Vec<(.+)>$/, '$1')
        const bounds = { u8: [0,255], u16: [0,65535], u32: [0,4294967295], i32: [-2147483648,2147483647], i64: [Number.MIN_SAFE_INTEGER,Number.MAX_SAFE_INTEGER], u64: [0,Number.MAX_SAFE_INTEGER] }[scalar]
        if (bounds) {
          const patch = { type: 'integer', minimum: bounds[0], maximum: bounds[1] }
          fields.set(`${match[1]}.${camel(field[1])}`, type.startsWith('Vec<') ? { items: patch } : patch)
        }
      }
    }
  }
  return fields
}

export async function generateInputs(root, typeDefDir) {
  // napi-derive 2.x records a full declaration; CLI 3.x adds export/declare itself.
  const records = readFileSync(join(typeDefDir, 'opensuite-node'), 'utf8').trim().split('\n').filter(Boolean).map(JSON.parse)
  for (const record of records) if (record.kind === 'fn') record.def = record.def.replace(/^export declare /, '')
  writeFileSync(join(typeDefDir, 'opensuite-node'), records.map((record) => JSON.stringify(record)).join('\n') + '\n')
  const generated = await generateTypeDef({ cwd: root, typeDefDir, noDtsHeader: true })
  const source = `// Generated from Rust N-API types. Do not edit.\nimport { Buffer } from 'node:buffer'\n\n${generated.dts}`
  writeFileSync(join(root, 'index.d.ts'), source + "\nexport * from './compatibility-types'\n")
  const contracts = inputContracts(source, rustIntegerFields(root))
  writeFileSync(join(root, 'operation-inputs.json'), JSON.stringify(contracts, null, 2) + '\n')
  return generated.exports
}
