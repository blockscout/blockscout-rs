const withReactSvg = require('next-react-svg')
const path = require('path')

module.exports = withReactSvg({
  include: path.resolve(__dirname, 'icons'),
  reactStrictMode: false,
  swcMinify: true,
  output: 'standalone',

  webpack(config, options) {
    return config
  }
})

