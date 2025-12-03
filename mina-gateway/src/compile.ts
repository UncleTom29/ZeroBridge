import { MinaGateway } from './mina-gateway.js';

async function main() {
  console.log('Compiling MinaGateway...');
  
  try {
    const { verificationKey } = await MinaGateway.compile();
    console.log('✅ Compilation complete!');
    console.log('Verification key hash:', verificationKey.hash.toString());
  } catch (error) {
    console.error('❌ Compilation failed:', error);
    throw error;
  }
}

main().catch(error => {
  console.error('Fatal error:', error);
  process.exit(1);
});