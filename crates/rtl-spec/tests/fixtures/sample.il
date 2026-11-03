# rtl2spec 自著測試語料(repo MIT):oracle proc 後 -readable 正典形子集。
# module il_counter:sync 復位(SWITCH 1-bit)+counter update;無 default 臂的
# 純組合 switch 在 il_latch 模組(G1 觸發);connect 自環在 il_loop 模組(G5)。
attribute \src "sample.v"
module \il_counter
  attribute \src "sample.v:2.8-2.11"
  wire width 8 output 2 \count
  attribute \src "sample.v:1.8-1.11"
  wire width 1 input 0 \clk
  attribute \src "sample.v:1.12-1.17"
  wire width 1 input 1 \reset
  attribute \src "sample.v:3.8-3.13"
  wire width 8 \d
  wire width 8 $0\count[7:0]
  wire width 8 $add$2_Y

  process $sample.v:5$proc$1
    assign { } { }
    assign $0\count[7:0] $add$2_Y [7:0]
    attribute \src "sample.v:6.5-6.20"
    switch \reset
    attribute \src "sample.v:6.9-6.14"
      case 1'1
        assign $0\count[7:0] 8'00000000
      case 
        assign $0\count[7:0] $add$2_Y [7:0]
    end
    sync posedge \clk
      update \count $0\count[7:0]
  end

  connect \d 8'00000000
end

attribute \src "sample.v"
module \il_latch
  attribute \src "sample.v:12.1-12.8"
  wire width 1 input 0 \sel
  attribute \src "sample.v:12.9-12.18"
  wire width 8 output 1 \q
  attribute \src "sample.v:12.19-12.24"
  wire width 8 input 2 \data

  process $sample.v:14$proc$2
    assign { } { }
    assign \q \data
    attribute \src "sample.v:15.5-15.30"
    switch \sel
    attribute \src "sample.v:15.10-15.13"
      case 1'1
        assign \q \data
    end
  end
end

attribute \src "sample.v"
module \il_loop
  attribute \src "sample.v:20.1-20.6"
  wire width 1 \a
  attribute \src "sample.v:20.7-20.12"
  wire width 1 output 0 \y

  connect \a \a
  connect \y \a
end
