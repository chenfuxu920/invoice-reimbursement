from paddleocr import PaddleOCR
from typing import Optional
import numpy as np
from PIL import Image
import io

_ocr_engine: Optional[PaddleOCR] = None

def get_ocr() -> PaddleOCR:
    global _ocr_engine
    if _ocr_engine is None:
        _ocr_engine = PaddleOCR(
            use_angle_cls=True,
            lang='ch',
            show_log=False,
        )
    return _ocr_engine

def recognize_image(image_bytes: bytes) -> list[dict]:
    """识别图片中的文字，返回结构化结果"""
    img = Image.open(io.BytesIO(image_bytes))
    img_array = np.array(img)

    ocr = get_ocr()
    result = ocr.ocr(img_array, cls=True)

    texts = []
    if result and result[0]:
        for line in result[0]:
            box = line[0]        # 坐标
            text = line[1][0]    # 文字
            confidence = line[1][1]  # 置信度
            texts.append({
                'text': text,
                'confidence': float(confidence),
                'box': box,
            })
    return texts
