export function formatEvaluations(evaluations) {
  return evaluations.map(({ value, output }) => `${output}${value}\n`).join("").trimEnd();
}
