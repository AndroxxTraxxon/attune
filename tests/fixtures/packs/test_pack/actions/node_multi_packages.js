const axios = require('axios');
const lodash = require('lodash');

console.log(JSON.stringify({
  success: true,
  axiosAvailable: Boolean(axios),
  lodashVersion: lodash.VERSION,
  sum: lodash.sum([1, 2, 3, 4, 5])
}));
