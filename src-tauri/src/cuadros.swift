// Extrae cuadros de un video con AVFoundation, que ya viene con macOS (no hace falta ffmpeg).
//
// Miniaturas en gris para el detector de parpadeos (parpadeos.rs):
//   cuadros <video.mp4> <cuadros-por-segundo> <ancho>
//   Salida (stdout): "ancho alto cantidad fps\n" y después los cuadros seguidos, ancho*alto bytes
//   cada uno (0 = negro, 255 = blanco).
//
// Capturas PNG de momentos concretos para la revisión con Claude (revision.rs):
//   cuadros --png <video.mp4> <ancho> <carpeta> <nombre>=<segundo> …
//   Escribe <carpeta>/<nombre> por cada par; los que no se puedan leer se omiten.
import AVFoundation
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

func fallar(_ texto: String, _ codigo: Int32) -> Never {
    FileHandle.standardError.write((texto + "\n").data(using: .utf8)!)
    exit(codigo)
}

let args = CommandLine.arguments
let modoPNG = args.count >= 2 && args[1] == "--png"
guard modoPNG ? args.count >= 5 : args.count == 4 else {
    fallar("uso: cuadros <video> <fps> <ancho> | cuadros --png <video> <ancho> <carpeta> <nombre>=<segundo>…", 2)
}
let video = modoPNG ? args[2] : args[1]
let asset = AVURLAsset(url: URL(fileURLWithPath: video))
let duracion = CMTimeGetSeconds((try? await asset.load(.duration)) ?? .zero)
guard duracion.isFinite, duracion > 0,
      let pista = try? await asset.loadTracks(withMediaType: .video).first,
      let (natural, transform) = try? await pista.load(.naturalSize, .preferredTransform) else {
    fallar("el video no tiene imagen", 1)
}
let tam = natural.applying(transform)
let gen = AVAssetImageGenerator(asset: asset)
gen.appliesPreferredTrackTransform = true
gen.requestedTimeToleranceBefore = .zero
gen.requestedTimeToleranceAfter = .zero

if modoPNG {
    guard let ancho = Int(args[3]), ancho > 0 else { fallar("ancho inválido", 2) }
    let carpeta = URL(fileURLWithPath: args[4])
    gen.maximumSize = CGSize(width: ancho, height: ancho * 4)
    for par in args.dropFirst(5) {
        guard let igual = par.lastIndex(of: "="), let s = Double(par[par.index(after: igual)...]) else { continue }
        let nombre = String(par[..<igual])
        // El último cuadro exacto a veces no existe: se pide un poco antes del final
        let t = CMTime(seconds: min(max(0, s), max(0, duracion - 0.05)), preferredTimescale: 600)
        guard let img = try? await gen.image(at: t).image,
              let dest = CGImageDestinationCreateWithURL(carpeta.appendingPathComponent(nombre) as CFURL,
                                                         UTType.png.identifier as CFString, 1, nil) else { continue }
        CGImageDestinationAddImage(dest, img, nil)
        CGImageDestinationFinalize(dest)
    }
    exit(0)
}

guard let fps = Double(args[2]), let ancho = Int(args[3]), fps > 0, ancho > 0 else { fallar("fps o ancho inválidos", 2) }
let alto = max(1, Int((Double(ancho) * abs(Double(tam.height)) / abs(Double(tam.width))).rounded()))
gen.maximumSize = CGSize(width: ancho * 4, height: alto * 4)

let n = Int(duracion * fps)
let salida = FileHandle.standardOutput
salida.write("\(ancho) \(alto) \(n) \(fps)\n".data(using: .utf8)!)
var buffer = [UInt8](repeating: 0, count: ancho * alto)
let gris = CGColorSpaceCreateDeviceGray()
for i in 0..<n {
    let t = CMTime(seconds: Double(i) / fps, preferredTimescale: 600)
    if let img = try? await gen.image(at: t).image,
       let ctx = CGContext(data: &buffer, width: ancho, height: alto, bitsPerComponent: 8,
                           bytesPerRow: ancho, space: gris, bitmapInfo: CGImageAlphaInfo.none.rawValue) {
        ctx.interpolationQuality = .medium
        ctx.draw(img, in: CGRect(x: 0, y: 0, width: ancho, height: alto))
    }
    // Si un cuadro no se pudo leer, se repite el anterior (no inventa un cambio)
    salida.write(Data(buffer))
}
