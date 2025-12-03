import { MinaGateway } from './mina-gateway.js';
async function main() {
    console.log('Compiling MinaGateway...');
    const { verificationKey } = await MinaGateway.compile();
    console.log('Compilation complete. Verification key hash:', verificationKey.hash.toString());
}
main().catch(error => {
    console.error('Compilation failed:', error);
    process.exit(1);
});
