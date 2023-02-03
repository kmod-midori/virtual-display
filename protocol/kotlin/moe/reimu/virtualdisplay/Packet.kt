// automatically generated by the FlatBuffers compiler, do not modify

package moe.reimu.virtualdisplay

import com.google.flatbuffers.BaseVector
import com.google.flatbuffers.BooleanVector
import com.google.flatbuffers.ByteVector
import com.google.flatbuffers.Constants
import com.google.flatbuffers.DoubleVector
import com.google.flatbuffers.FlatBufferBuilder
import com.google.flatbuffers.FloatVector
import com.google.flatbuffers.LongVector
import com.google.flatbuffers.StringVector
import com.google.flatbuffers.Struct
import com.google.flatbuffers.Table
import com.google.flatbuffers.UnionVector
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.sign

@Suppress("unused")
@kotlin.ExperimentalUnsignedTypes
class Packet : Table() {

    fun __init(_i: Int, _bb: ByteBuffer)  {
        __reset(_i, _bb)
    }
    fun __assign(_i: Int, _bb: ByteBuffer) : Packet {
        __init(_i, _bb)
        return this
    }
    val commandType : UByte
        get() {
            val o = __offset(4)
            return if(o != 0) bb.get(o + bb_pos).toUByte() else 0u
        }
    fun command(obj: Table) : Table? {
        val o = __offset(6); return if (o != 0) __union(obj, o + bb_pos) else null
    }
    companion object {
        fun validateVersion() = Constants.FLATBUFFERS_23_1_21()
        fun getRootAsPacket(_bb: ByteBuffer): Packet = getRootAsPacket(_bb, Packet())
        fun getRootAsPacket(_bb: ByteBuffer, obj: Packet): Packet {
            _bb.order(ByteOrder.LITTLE_ENDIAN)
            return (obj.__assign(_bb.getInt(_bb.position()) + _bb.position(), _bb))
        }
        fun createPacket(builder: FlatBufferBuilder, commandType: UByte, commandOffset: Int) : Int {
            builder.startTable(2)
            addCommand(builder, commandOffset)
            addCommandType(builder, commandType)
            return endPacket(builder)
        }
        fun startPacket(builder: FlatBufferBuilder) = builder.startTable(2)
        fun addCommandType(builder: FlatBufferBuilder, commandType: UByte) = builder.addByte(0, commandType.toByte(), 0)
        fun addCommand(builder: FlatBufferBuilder, command: Int) = builder.addOffset(1, command, 0)
        fun endPacket(builder: FlatBufferBuilder) : Int {
            val o = builder.endTable()
            return o
        }
        fun finishPacketBuffer(builder: FlatBufferBuilder, offset: Int) = builder.finish(offset)
        fun finishSizePrefixedPacketBuffer(builder: FlatBufferBuilder, offset: Int) = builder.finishSizePrefixed(offset)
    }
}