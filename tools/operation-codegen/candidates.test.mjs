import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, writeFile, mkdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import openapiTS, { astToString } from 'openapi-typescript';

const root = resolve(import.meta.dirname, '../..');
function run(command, args, cwd = root) {
  const result = spawnSync(command, args, { cwd, encoding: 'utf8', env: { ...process.env, CARGO_TARGET_DIR: join(root, 'target/operation-candidate') } });
  assert.equal(result.status, 0, result.stdout + result.stderr);
}

test('pinned generators retain actual production null, omission, flat, tagged, and numeric types', { timeout: 300000 }, async () => {
  const temporary = await mkdtemp(join(tmpdir(), 'operation-candidates-'));
  try {
    const document = JSON.parse(await readFile(join(root, 'contracts/operations/fixtures.openapi.json'), 'utf8'));
    const source = astToString(await openapiTS(document, { defaultNonNullable: false }));
    assert.doesNotMatch(source, /\bany\b/);
    await writeFile(join(temporary, 'schema.ts'), source);
    await writeFile(join(temporary, 'assertions.ts'), `import type { components } from './schema';
type Schemas = components['schemas'];
type Assert<T extends true> = T;
type Equal<A,B> = (<T>()=>T extends A?1:2) extends (<T>()=>T extends B?1:2) ? true : false;
type Issue = Assert<Equal<Schemas['ReportOutput']['issue'],9>>;
type Stale = Assert<null extends Schemas['EvidenceOutput']['stale'] ? true : false>;
type StaleRequired = Assert<{} extends Pick<Schemas['EvidenceOutput'],'stale'> ? false : true>;
type NodeOptional = Assert<{} extends Pick<Schemas['GetOutput'],'node'> ? true : false>;
type GetIdentity = Assert<Equal<Schemas['GetOutput']['operation'],'get'>>;
type EvidenceIdentity = Assert<Equal<Schemas['EvidenceOutput']['operation'],'evidence'>>;
type FoundFlat = Assert<Equal<Schemas['GetOutput']['found'],boolean>>;
type CreatedFlat = Assert<Equal<Schemas['PlanOutput']['created'],number>>;
type Cut = Assert<Equal<Schemas['EvidenceOutput']['reviews_has_more'],boolean>>;
// @ts-expect-error the standard issue is a closed integer literal
const issue: Schemas['ReportOutput']['issue'] = 8;
// @ts-expect-error required statement cannot be null
const input: Schemas['CheckStatementInput'] = {statement:null};
// @ts-expect-error tagged variants are closed
const node: Schemas['GraphNodeOutput']['node_type'] = 'invented';
`);
    run(process.execPath, [join(root, 'tools/operation-codegen/node_modules/typescript/bin/tsc'), '--strict', '--noEmit', '--skipLibCheck', '--target', 'es2022', join(temporary, 'assertions.ts')]);
    await mkdir(join(temporary, 'client'), { recursive: true });
    for (const name of ['client.ts', 'schema.ts']) await writeFile(join(temporary, 'client', name), await readFile(join(root, 'packages/provenance/src/generated', name)));
    await writeFile(join(temporary, 'client', 'assertions.ts'), `import { HttpClient, type OperationFailure, type components } from './client';
declare const client: HttpClient;
client.info({context:{repository:'first'},request:{}});
const context = {repository:'first',scope:'default'};
client.impact({context,request:{id:'rule_shared'}});
client.resolveSymbol({context,request:{file:'code.rs',symbol:null}});
client.evidence({context,request:{rule:'rule_shared',base:null}});
client.stale({context,request:{base:'HEAD',head:null}});
client.verificationRuns({context,request:{rule:null}});
client.verificationBindings({context,request:{}});
// @ts-expect-error verification lists do not accept freshness policy
client.verificationRuns({context:{...context,freshness:'catch_up'},request:{}});
// @ts-expect-error verification lists remain unbounded with a closed request
client.verificationBindings({context,request:{limit:1}});
client.get({context:{repository:'first',scope:'default',freshness:null},request:{node_type:'rule',id:'rule_shared'}});
// @ts-expect-error scoped get needs a scope
client.get({context:{repository:'first'},request:{node_type:'rule',id:'rule_shared'}});
// @ts-expect-error info context has no scope
client.info({context:{repository:'first',scope:'default'},request:{}});
type Assert<T extends true> = T;
type Closed = Assert<'invented' extends OperationFailure['error']['kind'] ? false : true>;
type StatementSubset = Assert<'stale' extends components['schemas']['CheckStatementFailureOutput']['error']['kind'] ? false : true>;
function facts(failure: OperationFailure): string | undefined {
  if (failure.operation === 'get' && failure.error.kind === 'stale') return failure.error.moved[0]?.stored;
}
`);
    run(process.execPath, [join(root, 'tools/operation-codegen/node_modules/typescript/bin/tsc'), '--strict', '--noEmit', '--skipLibCheck', '--target', 'es2022', join(temporary, 'client', 'assertions.ts')]);
    // Use the already built exporter. This test never starts a nested Cargo
    // build while another Cargo test holds the workspace target lock.
    await mkdir(join(temporary, 'src'), { recursive: true });
    run(join(root, 'target/debug/provenance-codegen'), ['rust', join(root, 'contracts/operations/fixtures.openapi.json'), join(temporary, 'src/generated')]);
    await writeFile(join(temporary, 'fixtures.json'), JSON.stringify(document['x-wire-fixtures']));
    await writeFile(join(temporary, 'Cargo.toml'), `[package]\nname="operation-generator-candidate"\nversion="0.0.0"\nedition="2021"\n[workspace]\n[dependencies]\nserde={version="=1.0.228",features=["derive"]}\nserde_json="=1.0.150"\nchrono={version="=0.4.45",features=["serde"]}\nuuid={version="=1.24.0",features=["serde"]}\nregress="=0.11.1"\n`);
    await writeFile(join(temporary, 'Cargo.lock'), await readFile(join(root, 'Cargo.lock')));
    const roundtrips = Object.keys(document['x-wire-fixtures']).map(name => `let decoded: ${name} = serde_json::from_value(fixtures["${name}"].clone()).unwrap(); assert_eq!(serde_json::to_value(decoded).unwrap(), fixtures["${name}"], "${name}");`).join('\n');
    await writeFile(join(temporary, 'src/lib.rs'), `#![allow(dead_code)]\ninclude!("generated/types.rs");\n#[test] fn actual_wire_values_round_trip() { let fixtures: serde_json::Value = serde_json::from_str(include_str!("../fixtures.json")).unwrap(); ${roundtrips} }\n#[test] fn numeric_issue_is_closed() { let mut report: serde_json::Value = serde_json::from_str(include_str!("../fixtures.json")).unwrap(); report["ReportOutput"]["issue"] = serde_json::json!(8); assert!(serde_json::from_value::<ReportOutput>(report["ReportOutput"].clone()).is_err()); }\n#[test] fn envelopes_reject_wrong_identity() { let fixtures: serde_json::Value = serde_json::from_str(include_str!("../fixtures.json")).unwrap(); let mut get = fixtures["GetOutput"].clone(); get["operation"] = serde_json::json!("evidence"); assert!(serde_json::from_value::<GetOutput>(get).is_err()); let mut evidence = fixtures["EvidenceOutput"].clone(); evidence["protocol_version"] = serde_json::json!(0); assert!(serde_json::from_value::<EvidenceOutput>(evidence).is_err()); }\n#[test] fn query_default_and_wire_types_survive_generation() { let search: SearchInput = serde_json::from_value(serde_json::json!({"text":"x"})).unwrap(); assert_eq!(serde_json::to_value(search.limit).unwrap(), serde_json::json!(50)); for limit in [serde_json::Value::Null,serde_json::json!("50")] { assert!(serde_json::from_value::<SearchInput>(serde_json::json!({"text":"x","limit":limit})).is_err(), "accepted limit {limit}"); } for limit in [1,200] { assert!(serde_json::from_value::<SearchInput>(serde_json::json!({"text":"x","limit":limit})).is_ok()); } assert!(serde_json::from_value::<SearchInput>(serde_json::json!({"text":"x","node_types":null})).is_err()); }`);
    run('cargo', ['test', '--quiet', '--offline', '--manifest-path', join(temporary, 'Cargo.toml')]);
  } finally { await rm(temporary, { recursive: true, force: true }); }
});
