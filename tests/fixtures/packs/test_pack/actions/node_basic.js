const fs = require('fs');
const input = fs.readFileSync(0, 'utf8').trim();
const params = input ? JSON.parse(input) : {};

console.log(JSON.stringify({
  success: true,
  message: params.message || 'Hello from Node.js',
  nodeVersion: process.version
}));
