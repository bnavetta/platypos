# -*- mode: ruby -*-
# vi: set ft=ruby :

Vagrant.configure("2") do |config|
  config.vm.box = "ubuntu/disco64"

  config.vm.network "forwarded_port", guest: 1234, host: 1234

  config.vm.provider "virtualbox" do |vb|
    # Display the VirtualBox GUI when booting the machine
    # vb.gui = true

    # https://medium.com/@njeremymiller/add-an-empty-optical-drive-to-oracle-virtualbox-instance-with-the-vagrantfile-523e8e9114be
    # Add an empty CD drive for inserting the Guest Additions disk
    # vb.customize ["storagectl", :id, "--add", "ide", "--name", "IDE"]
    # vb.customize ["storageattach", :id, "--storagectl", "IDE", "--port", "0", "--device", "1", "--type", "dvddrive", "--medium", "emptydrive"]
  end

  config.vm.provision "shell", inline: <<-SHELL
    apt-get update
    apt-get install -y qemu qemu-system-x86 gdb tmux curl direnv python-pip
    pip install click
  SHELL

  config.vm.provision "shell", privileged: false, inline: <<-SHELL
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain nightly
  SHELL

  config.vm.provision "file", source: ".bashrc", destination: ".bashrc"

  config.ssh.forward_x11 = true

end
