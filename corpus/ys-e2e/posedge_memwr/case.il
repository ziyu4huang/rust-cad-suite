module \top
  wire width 4 input 1 \d
  wire width 4 output 2 \q
  wire input 3 \c
  memory width 4 size 16 \m
  process \$proc
    assign \q \d
    sync posedge \c
      memwr \m \c \d 4'1111 1'1
  end
end
