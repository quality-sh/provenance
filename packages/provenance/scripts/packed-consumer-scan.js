import assert from "node:assert/strict";

export function assertPackedConsumerScan(output) {
  const report = JSON.parse(output);
  assert.deepEqual(report.warnings, []);
  assert.deepEqual(
    report.bindings.map(({ rule_id, file_path, item_name, verification }) => ({
      rule_id,
      file_path,
      item_name,
      verification,
    })),
    [
      {
        rule_id: "rule_packed_consumer_overtime",
        file_path: "rule-bindings.ts",
        item_name: "paysOvertime",
        verification: null,
      },
      {
        rule_id: "rule_packed_consumer_overtime",
        file_path: "rule-bindings.ts",
        item_name: "overtimeExamples",
        verification: "examples",
      },
    ],
  );
}
