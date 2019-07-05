class Oxidux < Formula
  desc 'Reverse proxy and process manager for web app development.'
  homepage 'https://github.com/jonmast/oxidux'
  version '0.3.0'

  if OS.mac?
      url "https://github.com/jonmast/oxidux/releases/download/v#{version}/oxidux-v#{version}-osx"
      sha256 '682c602998a960f0cdac1760885dcf1865500f3b3e21fe03485c89a3ad5b6e1c'
  elsif OS.linux?
      url "https://github.com/jonmast/oxidux/releases/download/v#{version}/oxidux-v#{version}-linux"
      sha256 '01b09b66bde13e3d57329fdad94d47387f218ba1525da139ca4d85bc96e0094c'
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
