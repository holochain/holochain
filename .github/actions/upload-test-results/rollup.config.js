const { default: resolve } = require('@rollup/plugin-node-resolve');
const { default: commonjs } = require('@rollup/plugin-commonjs');

module.exports = {
  input: 'index.js',
  output: {
    file: 'dist/index.js',
    format: 'cjs',
    sourcemap: false
  },
  plugins: [
    resolve({
      preferBuiltins: true
    }),
    commonjs()
  ]
};
