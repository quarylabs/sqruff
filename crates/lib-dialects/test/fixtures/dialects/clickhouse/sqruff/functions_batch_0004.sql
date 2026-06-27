-- seq_id 151 | base58Encode
SELECT base58Encode(1);

-- seq_id 152 | base64Decode
SELECT base64Decode('QQ==');

-- seq_id 153 | FROM_BASE64
SELECT FROM_BASE64('QQ==');

-- seq_id 154 | base64Encode
SELECT base64Encode('A');

-- seq_id 155 | TO_BASE64
SELECT TO_BASE64('A');

-- seq_id 156 | base64URLDecode
SELECT base64URLDecode('QQ');

-- seq_id 157 | base64URLEncode
SELECT base64URLEncode('A');

-- seq_id 158 | basename
SELECT basename('/tmp/a.txt');

-- seq_id 159 | bin
SELECT bin(1);

-- seq_id 160 | bitAnd
SELECT bitAnd(1, 1);

-- seq_id 161 | bitCount
SELECT bitCount(1);

-- seq_id 162 | bitHammingDistance
SELECT bitHammingDistance(1, 0);

-- seq_id 163 | bitmapAnd
SELECT bitmapAnd(1);

-- seq_id 164 | bitmapAndCardinality
SELECT bitmapAndCardinality(1);

-- seq_id 165 | bitmapAndnot
SELECT bitmapAndnot(1);

-- seq_id 166 | bitmapAndnotCardinality
SELECT bitmapAndnotCardinality(1);

-- seq_id 167 | bitmapBuild
SELECT bitmapBuild(1);

-- seq_id 168 | bitmapCardinality
SELECT bitmapCardinality(1);

-- seq_id 169 | bitmapContains
SELECT bitmapContains(1);

-- seq_id 170 | bitmapHasAll
SELECT bitmapHasAll(1);

-- seq_id 171 | bitmapHasAny
SELECT bitmapHasAny(1);

-- seq_id 172 | bitmapMax
SELECT bitmapMax(1);

-- seq_id 173 | bitmapMin
SELECT bitmapMin(1);

-- seq_id 174 | bitmapOr
SELECT bitmapOr(1);

-- seq_id 175 | bitmapOrCardinality
SELECT bitmapOrCardinality(1);

-- seq_id 176 | bitmapSubsetInRange
SELECT bitmapSubsetInRange(1);

-- seq_id 177 | bitmapSubsetLimit
SELECT bitmapSubsetLimit(1);

-- seq_id 178 | bitmapToArray
SELECT bitmapToArray(1);

-- seq_id 179 | bitmapTransform
SELECT bitmapTransform(1);

-- seq_id 180 | bitmapXor
SELECT bitmapXor(1);

-- seq_id 181 | bitmapXorCardinality
SELECT bitmapXorCardinality(1);

-- seq_id 182 | bitmaskToArray
SELECT bitmaskToArray(1);

-- seq_id 183 | bitmaskToList
SELECT bitmaskToList(1);

-- seq_id 184 | bitNot
SELECT bitNot(1);

-- seq_id 185 | bitOr
SELECT bitOr(1);

-- seq_id 186 | bitPositionsToArray
SELECT bitPositionsToArray(1);

-- seq_id 187 | bitRotateLeft
SELECT bitRotateLeft(1);

-- seq_id 188 | bitRotateRight
SELECT bitRotateRight(1);

-- seq_id 189 | bitShiftLeft
SELECT bitShiftLeft(1);

-- seq_id 190 | bitShiftRight
SELECT bitShiftRight(1);

-- seq_id 191 | bitSlice
SELECT bitSlice(1);

-- seq_id 192 | bitTest
SELECT bitTest(1);

-- seq_id 193 | bitTestAll
SELECT bitTestAll(1);

-- seq_id 194 | bitTestAny
SELECT bitTestAny(1);

-- seq_id 195 | bitXor
SELECT bitXor(1);

-- seq_id 196 | BLAKE3
SELECT BLAKE3('abc');

-- seq_id 197 | blockNumber
SELECT blockNumber();

-- seq_id 198 | blockSerializedSize
SELECT blockSerializedSize();

-- seq_id 199 | blockSize
SELECT blockSize();

-- seq_id 200 | boundingRatio
SELECT boundingRatio(1);
