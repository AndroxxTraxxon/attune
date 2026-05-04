const axios = require('axios');

console.log(JSON.stringify({
  success: true,
  axiosVersion: axios.VERSION || 'unknown'
}));
