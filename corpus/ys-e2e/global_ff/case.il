module \top
  wire width 4 input 1 \d
  wire width 4 output 2 \q
  process \$proc
    assign \q \d
    sync global
      update \q \d
  end
end
