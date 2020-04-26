const path = require('path')
const nodeExternals = require('webpack-node-externals');
const webpack = require('webpack');

module.exports = {
  mode: 'production',
  entry: path.resolve(__dirname, 'index.js'),
  externals: [
    nodeExternals({
      modulesDir: path.resolve(__dirname, './node_modules'),
    }),
  ],
  output: {
    path: path.resolve(__dirname, 'dist', 'umd'),
    filename: 'index.js',
    libraryTarget: 'umd',
    library: 'zokrates-js',
    globalObject: 'this'
  },
  resolve: {
    extensions: ['.js', '.ts', '.json'],
  },
  module: {
    rules: [
      {
        test: /\.ts$/,
        loader: "ts-loader",
        exclude:  /(node_modules|test)/,
      },
      {
        test: /\.wasm$/,
        loaders: ['wasm-loader'],
        type: 'javascript/auto',
      }
    ],
  },
  plugins: [
    new webpack.DefinePlugin({
      pkg: require('./package.json'),
      window: {},
    }),
  ]
};
