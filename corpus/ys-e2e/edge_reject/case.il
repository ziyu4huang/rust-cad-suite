module \top
  wire width 4 input 1 \d
  wire width 4 output 2 \q
  wire input 3 \c
  process \$proc
    assign \q \d
    sync edge \c
      update \q \d
  end
end
