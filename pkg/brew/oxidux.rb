class Oxidux < Formula
  desc 'Reverse proxy and process manager for web app development.'
  homepage 'https://github.com/jonmast/oxidux'
  version '0.4.0'

  if OS.mac?
    url "https://github.com/jonmast/oxidux/releases/download/v#{version}/oxidux-v#{version}-osx"
    sha256 '6f100afedd20172a2237c81148b155e456d50b73ff3594a43a38430d077d0146'
  elsif OS.linux?
    url "https://github.com/jonmast/oxidux/releases/download/v#{version}/oxidux-v#{version}-linux"
    sha256 '0f1e053e121b8c2435339b293332718c8cfb30e65d1344ace8fa6e00fbe6a5a7'
  end

  def install
    mv "oxidux-v#{version}-#{platform}", 'oxidux'

    bin.install 'oxidux'
  end

  def platform
    if OS.mac?
      'osx'
    else
      'linux'
    end
  end
end
