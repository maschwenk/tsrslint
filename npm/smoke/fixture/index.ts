// Fixture for npm/smoke.mjs: one diagnostic per rule in the smoke payload.
async function work(): Promise<number> {
    return 1;
}

export function run(items: number[]): void {
    work();
    delete items[0];
}
